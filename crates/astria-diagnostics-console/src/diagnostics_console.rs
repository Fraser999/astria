use std::{
    collections::HashMap,
    fs::Permissions,
    os::unix::fs::PermissionsExt as _,
    path::Path,
};

use tokio::{
    io::{
        AsyncBufReadExt,
        BufReader,
    },
    net::{
        UnixListener,
        UnixStream,
    },
    select,
    task::{
        JoinHandle,
        JoinSet,
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{
    debug,
    error,
    info,
    instrument,
    warn,
    Instrument,
};

use super::{
    actions,
    Action,
    ClientSession,
    Config,
    InitializationError,
    RegistrationError,
};

/// All subcommands registered must provide a unique display order <= `MAX_DISPLAY_ORDER`.
///
/// This value is chosen since the `help` subcommand added automatically is not provided with a
/// display order, and currently `clap` uses a default value of 999 for a such a subcommand. We
/// want the `help` subcommand listed last, and the other three subcommands provided by the
/// diagnostics console also listed after all external subcommands, hence the internal subcommands
/// are assigned display order values of 996 to 998.
const MAX_DISPLAY_ORDER: usize = 995;

/// The diagnostics console.
pub struct DiagnosticsConsole {
    config: Config,
    actions: HashMap<&'static str, Box<dyn Action + Send>>,
    /// Cancellation token that will cause server and client connections to exit when dropped.
    shutdown_token: CancellationToken,
}

impl DiagnosticsConsole {
    /// Creates a new diagnostics console using the provided `config`.
    #[must_use]
    pub fn new(config: Config, shutdown_token: CancellationToken) -> Self {
        let mut actions = HashMap::new();

        let log_filter_action = Box::new(actions::LogFilterAction::new());
        let _ = actions.insert(
            log_filter_action.name(),
            log_filter_action as Box<dyn Action + Send>,
        );

        let quit_action = Box::new(actions::QuitAction::new());
        let _ = actions.insert(quit_action.name(), quit_action as Box<dyn Action + Send>);
        DiagnosticsConsole {
            config,
            actions,
            shutdown_token,
        }
    }

    /// Registers an `Action` in the console.
    ///
    /// # Errors
    ///
    /// Returns an error if the [`Action::name()`] or [`Action::display_order()`] is the same as
    /// that of a previously-registered action.
    pub fn register_action<T: Action + Send + 'static>(
        &mut self,
        action: T,
    ) -> Result<(), RegistrationError> {
        if self.actions.contains_key(action.name()) {
            return Err(RegistrationError::duplicated_name(action.name()));
        }

        if let Some(conflicting_action) = self
            .actions
            .values()
            .find(|actn| actn.display_order() == action.display_order())
        {
            return Err(RegistrationError::duplicated_display_order(
                action.name(),
                conflicting_action.name(),
                action.display_order(),
            ));
        }

        if action.display_order() > MAX_DISPLAY_ORDER {
            return Err(RegistrationError::display_order_too_large(
                action.name(),
                action.display_order(),
                MAX_DISPLAY_ORDER,
            ));
        }

        let _ = self
            .actions
            .insert(action.name(), Box::new(action) as Box<dyn Action + Send>);
        Ok(())
    }

    /// Initializes the console.
    ///
    /// This sets up the listener, creating the socket at the configured path, and starts the main
    /// async loop in which connections are accepted and commands handled.
    ///
    /// Returns a JoinHandle for the main async loop.
    #[instrument(name = "diagnostics_console_init", skip_all)]
    pub fn run(self) -> Result<Option<JoinHandle<()>>, InitializationError> {
        if !self.config.enabled {
            info!("diagnostics console not enabled");
            return Ok(None);
        }

        let listener = listen(&self.config.socket_path, self.config.permissions)?;
        Ok(Some(tokio::spawn(serve(
            self.actions.clone(),
            listener,
            self.shutdown_token.clone(),
        ))))
    }
}

/// Sets up a Unix socket listener bound to a file at the given path with the given permissions.
fn listen<P: AsRef<Path>>(
    socket_path: P,
    permissions: u32,
) -> Result<UnixListener, InitializationError> {
    let socket_path = socket_path.as_ref();

    // Make a best-effort attempt to delete any stale file at the socket path.
    match std::fs::remove_file(socket_path) {
        Ok(()) => {
            debug!(
                socket_path = %socket_path.display(),
                "removed stale diagnostics console socket"
            );
        }
        Err(error) => {
            if !matches!(error.kind(), std::io::ErrorKind::NotFound) {
                warn!(
                    socket_path = %socket_path.display(), %error,
                    "error removing stale diagnostics console socket"
                );
            }
        }
    }

    let listener = UnixListener::bind(socket_path)
        .map_err(|source| InitializationError::bind(socket_path, source))?;
    std::fs::set_permissions(socket_path, Permissions::from_mode(permissions))
        .map_err(|source| InitializationError::set_permissions(socket_path, permissions, source))?;

    if let Some(path) = listener
        .local_addr()
        .ok()
        .and_then(|addr| addr.as_pathname().map(Path::to_path_buf))
    {
        info!(local_address = %path.display(), "diagnostics console listening");
    } else {
        warn!("diagnostics console listening on unnamed address");
    }

    Ok(listener)
}

async fn serve(
    actions: HashMap<&'static str, Box<dyn Action + Send>>,
    listener: UnixListener,
    shutdown_token: CancellationToken,
) {
    let listener_local_address = listener
        .local_addr()
        .ok()
        .and_then(|addr| addr.as_pathname().map(Path::to_path_buf));

    let child_shutdown_token = shutdown_token.child_token();
    let mut next_client_id: u64 = 0;

    let mut client_join_handles = JoinSet::new();
    loop {
        select!(
            biased;

            () = shutdown_token.cancelled() => {
                let _span = tracing::info_span!("diagnostics_console_serve").entered();
                info!("shutting down diagnostics console");
                break;
            },

            // No-op: just want to remove any joined tasks from the set.
            Some(_) = client_join_handles.join_next() => {}

            res = listener.accept() => match res {
                Ok((stream, _client_addr)) => {
                    let client_id = next_client_id;
                    let span = tracing::info_span!("diagnostics_console", client_id);
                    next_client_id = next_client_id.wrapping_add(1);
                    client_join_handles.spawn(
                        handle_client_connection(
                            actions.clone(),
                            stream,
                            child_shutdown_token.clone(),
                        )
                        .instrument(span),
                    );
                }
                Err(error) => {
                    info!(%error, "failed to accept incoming connection");
                }
            }
        );
    }

    // No-op: just want to ensure all tasks are joined.
    while client_join_handles.join_next().await.is_some() {}

    let _span = tracing::info_span!("diagnostics_console_serve").entered();
    if let Some(socket_path) = listener_local_address {
        match std::fs::remove_file(&socket_path) {
            Ok(()) => {
                debug!(
                    socket_path = %socket_path.display(),
                    "removed socket"
                );
            }
            Err(error) => {
                warn!(
                    socket_path = %socket_path.display(), %error,
                    "error removing socket"
                );
            }
        }
    } else {
        warn!("failed to get path of socket: couldn't remove");
    }
}

/// Handler for a client connection.
///
/// The core loop for the diagnostics console: reads commands via unix socket and processes them.
///
/// # Security
///
/// This will buffer an unlimited amount of data if no newline is encountered in the input stream.
/// For this reason, ensure that only trusted clients connect to the socket.
async fn handle_client_connection(
    actions: HashMap<&'static str, Box<dyn Action + Send>>,
    stream: UnixStream,
    shutdown_token: CancellationToken,
) {
    info!("accepted new connection");

    let (reader, writer) = stream.into_split();
    let mut client_session = ClientSession::new(actions, writer);
    let mut lines = BufReader::new(reader).lines();
    loop {
        select!(
            () = shutdown_token.cancelled() => {
                info!("shutdown signalled: closing connection");
                return;
            }

            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        if let NextStep::Quit = client_session.handle_line(line).await {
                            info!("client sent quit: closing connection");
                            return;
                        }
                    }
                    Ok(None) => {
                        info!("client closed connection");
                        return;
                    }
                    Err(error) => {
                        if let std::io::ErrorKind::ConnectionReset = error.kind() {
                            info!("client closed connection");
                        } else {
                            error!(%error, "error reading line");
                            return;
                        }
                    }
                }
            }
        );
    }
}

/// An indication of what the associated client session should do next.
pub enum NextStep {
    /// The client session should continue, awaiting further input from the client.
    Continue,
    /// The client session should be terminated.
    Quit,
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{
        FileTypeExt as _,
        PermissionsExt as _,
    };

    use tokio::{
        io::{
            AsyncReadExt as _,
            AsyncWriteExt as _,
        },
        net::UnixStream,
    };

    use super::*;

    #[test]
    fn default_display_order() {
        assert_eq!(
            MAX_DISPLAY_ORDER + 4,
            clap::Command::new("test").get_display_order()
        );
    }

    #[tokio::test]
    async fn should_listen() {
        const TEST_MESSAGE: &[u8] = b"hello, world!";

        let tmpdir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let socket_path = tmpdir.path().join(config.socket_path);
        let listener = listen(&socket_path, config.permissions).unwrap();

        // Check the permissions have been set correctly.
        let metadata = std::fs::metadata(&socket_path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, config.permissions);

        // Connect and send.
        tokio::spawn(async move {
            let mut stream = UnixStream::connect(socket_path).await.unwrap();
            stream.write_all(TEST_MESSAGE).await.unwrap();
        });

        // Accept and read.
        let (mut stream, _socket_addr) = listener.accept().await.unwrap();
        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer).await.unwrap();

        assert_eq!(TEST_MESSAGE, buffer.as_slice());
    }

    #[tokio::test]
    async fn should_remove_stale_socket() {
        let tmpdir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let socket_path = tmpdir.path().join(config.socket_path);

        // Create a file at the socket path before listening.
        std::fs::write(&socket_path, b"a").unwrap();
        let metadata = std::fs::metadata(&socket_path).unwrap();
        assert!(!metadata.file_type().is_socket());

        // Creating the listener should remove the stale file.
        let _listener = listen(&socket_path, 0o000).unwrap();
        let metadata = std::fs::metadata(&socket_path).unwrap();
        assert!(metadata.file_type().is_socket());
    }
}
