use std::borrow::Cow;

use astria_core::{
    primitive::v1::{
        asset::Denom as DomainDenom,
        Address as DomainAddress,
    },
    sequencerblock::v1alpha1::block::Deposit as DomainDeposit,
};
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};

use super::{
    AddressBytes,
    IbcPrefixedDenom,
    RollupId,
    StoredValue,
    TracePrefixedDenom,
};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct Address<'a> {
    bytes: AddressBytes<'a>,
    prefix: Cow<'a, str>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub(crate) enum Denom<'a> {
    TracePrefixed(TracePrefixedDenom<'a>),
    IbcPrefixed(IbcPrefixedDenom<'a>),
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub(crate) struct Deposit<'a> {
    bridge_address: Address<'a>,
    rollup_id: RollupId<'a>,
    amount: u128,
    asset: Denom<'a>,
    destination_chain_address: Cow<'a, str>,
}

impl<'a> From<&'a DomainDeposit> for Deposit<'a> {
    fn from(deposit: &'a DomainDeposit) -> Self {
        let bridge_address = Address {
            bytes: deposit.bridge_address().bytes().into(),
            prefix: Cow::Borrowed(deposit.bridge_address().prefix()),
        };
        let asset = match deposit.asset() {
            DomainDenom::TracePrefixed(denom) => Denom::TracePrefixed(denom.into()),
            DomainDenom::IbcPrefixed(denom) => Denom::IbcPrefixed(denom.into()),
        };
        Deposit {
            bridge_address,
            rollup_id: RollupId::from(deposit.rollup_id()),
            amount: deposit.amount(),
            asset,
            destination_chain_address: Cow::Borrowed(deposit.destination_chain_address()),
        }
    }
}

impl<'a> From<Deposit<'a>> for DomainDeposit {
    fn from(deposit: Deposit<'a>) -> Self {
        let address = DomainAddress::unchecked_from_parts(
            deposit.bridge_address.bytes.into(),
            &deposit.bridge_address.prefix,
        );
        let asset = match deposit.asset {
            Denom::TracePrefixed(denom) => DomainDenom::TracePrefixed(denom.into()),
            Denom::IbcPrefixed(denom) => DomainDenom::IbcPrefixed(denom.into()),
        };
        DomainDeposit::new(
            address,
            deposit.rollup_id.into(),
            deposit.amount,
            asset,
            deposit.destination_chain_address.into(),
        )
    }
}

impl<'a> TryFrom<StoredValue<'a>> for Deposit<'a> {
    type Error = anyhow::Error;

    fn try_from(value: StoredValue<'a>) -> Result<Self, Self::Error> {
        let StoredValue::Deposit(deposit) = value else {
            return Err(super::type_mismatch("deposit", &value));
        };
        Ok(deposit)
    }
}
