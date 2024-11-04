use std::path::{
    Path,
    PathBuf,
};

use thiserror::Error;

/// Error setting up the diagnostics console's Unix socket listener.
#[derive(Debug, Error)]
#[error("could not set up diagnostics console listener")]
#[expect(clippy::module_name_repetitions, reason = "this name makes sense")]
pub enum InitializationError {
    /// Failed to bind to the socket.
    #[error("failed to bind to socket at {}", socket_path.display())]
    Bind {
        /// The path to the socket file.
        socket_path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// Failed to set the permissions on the socket.
    #[error("failed to set permissions of {} to {permissions}", socket_path.display())]
    SetPermissions {
        /// The path to the socket file.
        socket_path: PathBuf,
        /// The requested permissions.
        permissions: u32,
        /// The underlying I/O error.
        source: std::io::Error,
    },
}

impl InitializationError {
    pub(crate) fn bind<P: AsRef<Path>>(path: P, source: std::io::Error) -> Self {
        Self::Bind {
            socket_path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub(crate) fn set_permissions<P: AsRef<Path>>(
        path: P,
        permissions: u32,
        source: std::io::Error,
    ) -> Self {
        Self::SetPermissions {
            socket_path: path.as_ref().to_path_buf(),
            permissions,
            source,
        }
    }
}

/// Error registering an action.
#[derive(Debug, Error)]
#[error("could not register the given action")]
#[expect(clippy::module_name_repetitions, reason = "this name makes sense")]
pub enum RegistrationError {
    /// The given action's name has been assigned to an already-registered action.
    #[error("action `{action_name}` has already been registered")]
    DuplicatedName {
        /// The action's name.
        action_name: &'static str,
    },
    /// The given action's display order has been assigned to an already-registered action.
    #[error(
        "action `{action_name}` has a display order of {display_order}, but this is also used by \
         `{conflicting_action_name}`"
    )]
    DuplicatedDisplayOrder {
        /// The name of the action being registered.
        action_name: &'static str,
        /// The name of the action already registered.
        conflicting_action_name: &'static str,
        /// The action's display order.
        display_order: usize,
    },
    /// The given action's display order is higher than the maximum supported.
    #[error(
        "action `{action_name}` has a display order of {display_order}, which exceeds the maximum \
         supported value of {maximum_display_order}"
    )]
    DisplayOrderTooLarge {
        /// The name of the action being registered.
        action_name: &'static str,
        /// The action's display order.
        display_order: usize,
        /// The action's display order.
        maximum_display_order: usize,
    },
}

impl RegistrationError {
    pub(crate) fn duplicated_name(action_name: &'static str) -> Self {
        Self::DuplicatedName {
            action_name,
        }
    }

    pub(crate) fn duplicated_display_order(
        action_name: &'static str,
        conflicting_action_name: &'static str,
        display_order: usize,
    ) -> Self {
        Self::DuplicatedDisplayOrder {
            action_name,
            conflicting_action_name,
            display_order,
        }
    }

    pub(crate) fn display_order_too_large(
        action_name: &'static str,
        display_order: usize,
        maximum_display_order: usize,
    ) -> Self {
        Self::DisplayOrderTooLarge {
            action_name,
            display_order,
            maximum_display_order,
        }
    }
}
