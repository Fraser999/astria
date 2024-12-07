use std::path::{
    Path,
    PathBuf,
};

use super::{
    upgrade1,
    Change,
    Upgrade1,
};
use crate::{
    generated::upgrades::v1::Upgrades as RawUpgrades,
    Protobuf,
};

#[derive(Clone, Debug, Default)]
pub struct Upgrades {
    upgrade_1: Option<Upgrade1>,
}

impl Upgrades {
    /// Returns a new `Upgrades` by reading the file at `path` and decoding from JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if reading, parsing or converting from raw (protobuf) upgrades fails.
    #[cfg(feature = "serde")]
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let contents = std::fs::read(path.as_ref())
            .map_err(|source| Error::read_file(source, path.as_ref().to_path_buf()))?;
        let raw_upgrades = serde_json::from_slice::<RawUpgrades>(&contents)
            .map_err(|source| Error::json_decode(source, path.as_ref().to_path_buf()))?;
        let upgrade_1 = raw_upgrades
            .upgrade_1
            .map(|raw_upgrade_1| {
                Upgrade1::try_from_raw(raw_upgrade_1)
                    .map_err(|source| Error::convert_upgrade_1(source, path.as_ref().to_path_buf()))
            })
            .transpose()?;
        Ok(Self {
            upgrade_1,
        })
    }

    /// Returns a verbose JSON-encoded string of `self`.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails.
    #[cfg(feature = "serde")]
    pub fn to_json_pretty(&self) -> Result<String, Error> {
        let Upgrades {
            upgrade_1,
        } = self.clone();

        let raw_upgrades = RawUpgrades {
            upgrade_1: upgrade_1.map(Upgrade1::into_raw),
        };
        serde_json::to_string_pretty(&raw_upgrades).map_err(Error::json_encode)
    }

    #[must_use]
    pub fn upgrade_1(&self) -> Option<&Upgrade1> {
        self.upgrade_1.as_ref()
    }
}

/// An error when constructing or JSON-encoding an [`Upgrades`].
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(ErrorKind);

impl Error {
    fn read_file(source: std::io::Error, path: PathBuf) -> Self {
        Self(ErrorKind::ReadFile {
            source,
            path,
        })
    }

    fn json_decode(source: serde_json::Error, path: PathBuf) -> Self {
        Self(ErrorKind::JsonDecode {
            source,
            path,
        })
    }

    fn json_encode(source: serde_json::Error) -> Self {
        Self(ErrorKind::JsonEncode {
            source,
        })
    }

    fn convert_upgrade_1(source: upgrade1::Error, path: PathBuf) -> Self {
        Self(ErrorKind::ConvertUpgrade1 {
            source,
            path,
        })
    }
}

#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error("failed to read file at `{}`", .path.display())]
    ReadFile {
        source: std::io::Error,
        path: PathBuf,
    },

    #[error("failed to json-decode file at `{}`", .path.display())]
    JsonDecode {
        source: serde_json::Error,
        path: PathBuf,
    },

    #[error("failed to json-encode upgrades")]
    JsonEncode { source: serde_json::Error },

    #[error("error converting `upgrade_1` in `{}`", .path.display())]
    ConvertUpgrade1 {
        source: upgrade1::Error,
        path: PathBuf,
    },
}
