use std::{
    borrow::Cow,
    fmt::{
        self,
        Debug,
        Formatter,
    },
};

use astria_core::upgrades::v1::ChangeHash as DomainChangeHash;
use astria_eyre::eyre::bail;
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};
use telemetry::display::base64;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct Value<'a>(ValueImpl<'a>);

#[derive(Debug, BorshSerialize, BorshDeserialize)]
enum ValueImpl<'a> {
    ChangeHash(ChangeHash<'a>),
}

#[derive(BorshSerialize, BorshDeserialize)]
pub(in crate::upgrades) struct ChangeHash<'a>(Cow<'a, [u8; DomainChangeHash::LENGTH]>);

impl<'a> Debug for ChangeHash<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", base64(self.0.as_slice()))
    }
}

impl<'a> From<&'a DomainChangeHash> for ChangeHash<'a> {
    fn from(value: &'a DomainChangeHash) -> Self {
        ChangeHash(Cow::Borrowed(value.as_bytes()))
    }
}

impl<'a> From<ChangeHash<'a>> for DomainChangeHash {
    fn from(change_hash: ChangeHash<'a>) -> Self {
        DomainChangeHash::new(change_hash.0.into_owned())
    }
}

impl<'a> From<ChangeHash<'a>> for crate::storage::StoredValue<'a> {
    fn from(change_hash: ChangeHash<'a>) -> Self {
        crate::storage::StoredValue::Upgrades(Value(ValueImpl::ChangeHash(change_hash)))
    }
}

impl<'a> TryFrom<crate::storage::StoredValue<'a>> for ChangeHash<'a> {
    type Error = astria_eyre::eyre::Error;

    fn try_from(value: crate::storage::StoredValue<'a>) -> Result<Self, Self::Error> {
        let crate::storage::StoredValue::Upgrades(Value(ValueImpl::ChangeHash(change_hash))) =
            value
        else {
            bail!("upgrades stored value type mismatch: expected change hash, found {value:?}");
        };
        Ok(change_hash)
    }
}
