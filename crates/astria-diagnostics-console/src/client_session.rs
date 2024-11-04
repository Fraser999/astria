use std::{
    collections::HashMap,
    fmt::{
        self,
        Debug,
        Display,
        Formatter,
    },
    sync::{
        Arc,
        Mutex,
    },
};

use clap::{
    error::ErrorKind,
    Command,
};
use serde::Serialize;
use tokio::{
    io::AsyncWriteExt,
    net::unix::OwnedWriteHalf,
};
use tracing::{
    error,
    info,
    instrument,
    trace,
    warn,
};

use super::{
    actions::{
        ConfigAction,
        QuitAction,
    },
    Action,
    NextStep,
    OutputFormat,
    Response,
};

/// The settings for a single client session.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize)]
pub(crate) struct SessionSettings {
    /// If false, suppress sending the operation outcome to the client.
    pub(crate) show_outcome: bool,
    /// Output format to send to client.
    pub(crate) output_format: OutputFormat,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            show_outcome: true,
            output_format: OutputFormat::default(),
        }
    }
}

impl Display for SessionSettings {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "  show-outcome: {}", self.show_outcome)?;
        write!(f, "  output-format: {}", self.output_format)
    }
}

/// A single client session.
pub(crate) struct ClientSession {
    settings: Arc<Mutex<SessionSettings>>,
    /// The registered actions that can be executed.
    actions: HashMap<&'static str, Box<dyn Action + Send>>,
    /// The clap Command constructed from all the actions' subcommands.
    command: Command,
    /// The writer half of the socket, for sending responses to the client.
    writer: OwnedWriteHalf,
}

impl ClientSession {
    pub(crate) fn new(
        mut actions: HashMap<&'static str, Box<dyn Action + Send>>,
        writer: OwnedWriteHalf,
    ) -> Self {
        let settings = Arc::new(Mutex::new(SessionSettings::default()));

        // We can only add the config action here as it needs a copy of the session settings.
        let config_action = ConfigAction::new(settings.clone());
        let _ = actions.insert(
            config_action.name(),
            Box::new(config_action) as Box<dyn Action + Send>,
        );

        let mut command = Command::new("")
            .no_binary_name(true)
            .help_template("{all-args}");
        for action in actions.values() {
            command = command.subcommand(action.get_subcommand());
        }

        Self {
            settings,
            actions,
            command,
            writer,
        }
    }

    #[instrument(skip_all)]
    pub(crate) async fn handle_line(&mut self, line: String) -> NextStep {
        trace!(%line, "line received");

        let settings = match self.settings() {
            Ok(settings) => settings,
            Err(response) => {
                self.send_response(response).await;
                return NextStep::Continue;
            }
        };

        let mut action = match self.parse_line(&line) {
            ParsedLine::Action(action) => action.clone(),
            ParsedLine::Help(help_string) => {
                self.send_response(Response::success(settings.output_format, "", &help_string))
                    .await;
                return NextStep::Continue;
            }
            ParsedLine::Error(error_msg) => {
                self.send_response(Response::failure(error_msg)).await;
                return NextStep::Continue;
            }
        };
        info!(action = %action.name(), "processing action");

        let response = action.execute(settings.output_format).await;
        self.send_response(response).await;

        if action.as_any().downcast_ref::<QuitAction>().is_some() {
            return NextStep::Quit;
        }
        NextStep::Continue
    }

    fn parse_line(&self, line: &str) -> ParsedLine {
        let Some(args) = shlex::split(line) else {
            warn!("failed to parse client input as posix shell syntax");
            return ParsedLine::Error("failed to parse input as posix shell syntax".to_string());
        };

        let arg_matches = match self.command.clone().try_get_matches_from(args) {
            Ok(arg_matches) => arg_matches,
            Err(error) if error.kind() == ErrorKind::DisplayHelp => {
                return ParsedLine::Help(error.to_string());
            }
            Err(error) => {
                warn!(
                    error = error.to_string().lines().next().unwrap_or_default().trim(),
                    "failed to parse client action"
                );
                return ParsedLine::Error(error.to_string());
            }
        };

        let Some((subcommand_name, subcommand_arg_matches)) = arg_matches.subcommand() else {
            return ParsedLine::Help(self.command.clone().render_help().to_string());
        };

        let Some(mut action) = self.actions.get(subcommand_name).cloned() else {
            return ParsedLine::Error(format!(
                "internal error: failed to find action '{subcommand_name}'"
            ));
        };

        match action.set_options(subcommand_arg_matches) {
            Ok(()) => ParsedLine::Action(action),
            Err(error) if error.kind() == ErrorKind::MissingSubcommand => ParsedLine::Error(
                format!("{}\n{}", error, action.get_subcommand().render_long_help()),
            ),
            Err(error) => {
                error!(%error, "failed to set options on action '{subcommand_name}'");
                ParsedLine::Error(format!("internal error: failed to set options: {error}"))
            }
        }
    }

    /// Sends the given response to the client.
    ///
    /// If the session is in quiet mode (`show_outcome` is false), only the body of the `Success`
    /// variant is sent; nothing is sent if the response is `Failure`. If not in quiet mode, the
    /// full response is sent regardless of the variant.
    async fn send_response(&mut self, response: Response) {
        let Ok(settings) = self.settings() else {
            error!("failed sending response");
            return;
        };
        let mut output = response.into_string(settings.show_outcome);
        if settings.output_format == OutputFormat::HumanReadable {
            output.push('\n');
        }
        if let Err(error) = self.writer.write_all(output.as_bytes()).await {
            warn!(%error, "failed sending response");
        }
    }

    fn settings(&self) -> Result<SessionSettings, Response> {
        self.settings.lock().map(|guard| *guard).map_err(|error| {
            error!(%error, "failed to lock settings");
            Response::failure("internal error: failed to get settings")
        })
    }
}

enum ParsedLine {
    Action(Box<dyn Action + Send>),
    Help(String),
    Error(String),
}
