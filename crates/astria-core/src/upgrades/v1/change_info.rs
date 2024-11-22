use std::fmt::{
    self,
    Display,
    Formatter,
};

use super::{
    ChangeHash,
    ChangeName,
};

/// The common details of a given upgrade change.
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct ChangeInfo {
    pub activation_height: u64,
    pub name: ChangeName,
    pub app_version: u64,
    pub hash: ChangeHash,
}

impl Display for ChangeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "upgrade change `{}` with activation height {}, app version {}, change hash {}",
            self.name, self.activation_height, self.app_version, self.hash
        )
    }
}
