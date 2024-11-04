use std::path::PathBuf;

use serde::{
    Deserialize,
    Serialize,
};

/// Diagnostics console configuration.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    /// Whether or not the diagnostics console is enabled.
    pub enabled: bool,
    /// Path to the socket.
    pub socket_path: PathBuf,
    /// Permissions to apply to the socket.
    pub permissions: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            socket_path: ".diagnostics.socket".into(),
            permissions: 0o600,
        }
    }
}
