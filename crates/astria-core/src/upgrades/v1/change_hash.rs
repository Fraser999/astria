use std::fmt::{
    self,
    Debug,
    Display,
    Formatter,
};

use base64::{
    display::Base64Display,
    engine::general_purpose::STANDARD,
};

/// A SHA256 digest of a Borsh-encoded upgrade change.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ChangeHash([u8; 32]);

impl ChangeHash {
    pub const LENGTH: usize = 32;

    #[must_use]
    pub const fn new(digest: [u8; Self::LENGTH]) -> Self {
        Self(digest)
    }

    #[must_use]
    pub fn bytes(self) -> [u8; Self::LENGTH] {
        self.0
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }
}

impl Display for ChangeHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Base64Display::new(&self.0, &STANDARD))
    }
}

impl Debug for ChangeHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ChangeHash({self})")
    }
}
