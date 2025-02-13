use std::fmt::{
    self,
    Debug,
    Formatter,
};

use astria_core::{
    primitive::v1::{
        asset::Denom,
        Address,
        Bech32,
        ADDRESS_LEN,
    },
    protocol::{
        memos::v1::Ics20WithdrawalFromRollup,
        transaction::v1::action::Ics20Withdrawal,
    },
};
use astria_eyre::{
    anyhow_to_eyre,
    eyre::{
        bail,
        ensure,
        OptionExt as _,
        Result,
        WrapErr as _,
    },
};
use cnidarium::{
    StateRead,
    StateWrite,
};
use ibc_types::core::channel::{
    ChannelId,
    PortId,
};
use penumbra_ibc::component::packet::{
    IBCPacket,
    SendPacketRead as _,
    SendPacketWrite as _,
    Unchecked,
};
use penumbra_proto::core::component::ibc::v1::FungibleTokenPacketData;
use tracing::{
    instrument,
    Level,
};

use super::TransactionSignerAddressBytes;
use crate::{
    accounts::StateWriteExt as _,
    address::StateReadExt as _,
    app::StateReadExt as _,
    bridge::{
        StateReadExt as _,
        StateWriteExt,
    },
    ibc::{
        StateReadExt as _,
        StateWriteExt as _,
    },
};

pub(crate) struct CheckedIcs20Withdrawal {
    amount: u128,
    denom: Denom,
    fee_asset: Denom,
    withdrawal_address: [u8; ADDRESS_LEN],
    bridge_address_and_rollup_withdrawal: Option<(Address, Ics20WithdrawalFromRollup)>,
    ibc_packet: IBCPacket<Unchecked>,
    tx_signer: TransactionSignerAddressBytes,
}

impl CheckedIcs20Withdrawal {
    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn new<S: StateRead>(
        action: Ics20Withdrawal,
        tx_signer: [u8; ADDRESS_LEN],
        state: S,
    ) -> Result<Self> {
        state
            .ensure_base_prefix(&action.return_address)
            .await
            .wrap_err("failed to verify that return address has permitted base prefix")?;

        ensure!(action.timeout_time != 0, "timeout time must be non-zero");
        ensure!(action.amount > 0, "amount must be greater than zero");
        let withdrawal_address = action
            .bridge_address
            .as_ref()
            .map_or(tx_signer, |address| address.bytes());
        let bridge_address_and_rollup_withdrawal = if let Some(bridge_address) =
            action.bridge_address
        {
            state
                .ensure_base_prefix(&bridge_address)
                .await
                .wrap_err("bridge address has an unsupported prefix")?;
            let parsed_bridge_memo: Ics20WithdrawalFromRollup = serde_json::from_str(&action.memo)
                .wrap_err("failed to parse memo for ICS bound bridge withdrawal")?;
            ensure!(
                !parsed_bridge_memo.rollup_return_address.is_empty(),
                "rollup return address must be non-empty",
            );
            ensure!(
                parsed_bridge_memo.rollup_return_address.len() <= 256,
                "rollup return address must be no more than 256 bytes",
            );
            ensure!(
                !parsed_bridge_memo.rollup_withdrawal_event_id.is_empty(),
                "rollup withdrawal event id must be non-empty",
            );
            ensure!(
                parsed_bridge_memo.rollup_withdrawal_event_id.len() <= 256,
                "rollup withdrawal event id must be no more than 256 bytes",
            );
            ensure!(
                parsed_bridge_memo.rollup_block_number != 0,
                "rollup block number must be non-zero",
            );
            Some((bridge_address, parsed_bridge_memo))
        } else {
            None
        };

        let amount = action.amount;
        let denom = action.denom.clone();
        let fee_asset = action.fee_asset.clone();
        let ibc_packet = create_ibc_packet_from_withdrawal(action, &state).await?;
        let tx_signer = TransactionSignerAddressBytes::from(tx_signer);

        let checked_action = Self {
            amount,
            denom,
            fee_asset,
            withdrawal_address,
            bridge_address_and_rollup_withdrawal,
            ibc_packet,
            tx_signer,
        };
        checked_action.run_mutable_checks(state).await?;

        Ok(checked_action)
    }

    #[instrument(skip_all, err(level = Level::DEBUG))]
    pub(super) async fn execute<S: StateWrite>(&self, mut state: S) -> Result<()> {
        self.run_mutable_checks(&state).await?;
        if let Some((_bridge_address, rollup_withdrawal)) =
            &self.bridge_address_and_rollup_withdrawal
        {
            state
                .put_withdrawal_event_block_for_bridge_account(
                    &self.withdrawal_address,
                    &rollup_withdrawal.rollup_withdrawal_event_id,
                    rollup_withdrawal.rollup_block_number,
                )
                .wrap_err("failed to write withdrawal event block to storage")?;
        }

        let current_timestamp = state
            .get_block_timestamp()
            .await
            .wrap_err("failed to read block timestamp from storage")?;
        // `IBCPacket<Unchecked>` doesn't implement `Clone` - manually clone it.
        let unchecked_packet = IBCPacket::new(
            self.ibc_packet.source_port().clone(),
            self.ibc_packet.source_channel().clone(),
            *self.ibc_packet.timeout_height(),
            self.ibc_packet.timeout_timestamp(),
            self.ibc_packet.data().to_vec(),
        );
        let checked_packet = state
            .send_packet_check(unchecked_packet, current_timestamp)
            .await
            .map_err(anyhow_to_eyre)
            .wrap_err("ibc packet failed send check")?;

        state
            .decrease_balance(&self.withdrawal_address, &self.denom, self.amount)
            .await
            .wrap_err("failed to decrease sender or bridge balance")?;

        // If we're the source, move tokens to the escrow account, otherwise the tokens are just
        // burned.
        if is_source(
            checked_packet.source_port(),
            checked_packet.source_channel(),
            &self.denom,
        ) {
            let channel_balance = state
                .get_ibc_channel_balance(self.ibc_packet.source_channel(), &self.denom)
                .await
                .wrap_err("failed to read channel balance from storage")?;

            state
                .put_ibc_channel_balance(
                    self.ibc_packet.source_channel(),
                    &self.denom,
                    channel_balance
                        .checked_add(self.amount)
                        .ok_or_eyre("overflow when adding to channel balance")?,
                )
                .wrap_err("failed to write channel balance to storage")?;
        }

        state.send_packet_execute(checked_packet).await;
        Ok(())
    }

    async fn run_mutable_checks<S: StateRead>(&self, state: S) -> Result<()> {
        if let Some((bridge_address, rollup_withdrawal)) =
            &self.bridge_address_and_rollup_withdrawal
        {
            let Some(withdrawer) = state
                .get_bridge_account_withdrawer_address(bridge_address)
                .await
                .wrap_err("failed to read bridge withdrawer address from storage")?
            else {
                bail!("bridge account does not have an associated withdrawer address in storage");
            };

            ensure!(
                &withdrawer == self.tx_signer.as_bytes(),
                "transaction signer not authorized to perform ics20 bridge withdrawal"
            );

            if let Some(existing_block_num) = state
                .get_withdrawal_event_block_for_bridge_account(
                    &self.withdrawal_address,
                    &rollup_withdrawal.rollup_withdrawal_event_id,
                )
                .await
                .wrap_err("withdrawal event already processed")?
            {
                bail!(
                    "withdrawal event ID `{}` used by block number {existing_block_num}",
                    rollup_withdrawal.rollup_withdrawal_event_id
                );
            }
        } else if state
            .is_a_bridge_account(&self.tx_signer)
            .await
            .wrap_err("failed to establish whether the signer is a bridge account")?
        {
            bail!("signer cannot be a bridge address if bridge address is not set");
        }

        Ok(())
    }
}

impl Debug for CheckedIcs20Withdrawal {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedIcs20Withdrawal")
            .field("amount", &self.amount)
            .field("denom", &self.denom)
            .field("fee_asset", &self.fee_asset)
            .field("withdrawal_address", &self.withdrawal_address)
            .field(
                "bridge_address_and_rollup_withdrawal",
                &self.bridge_address_and_rollup_withdrawal,
            )
            .field("ibc_packet.source_port", self.ibc_packet.source_port())
            .field(
                "ibc_packet.source_channel",
                self.ibc_packet.source_channel(),
            )
            .field(
                "ibc_packet.timeout_height",
                self.ibc_packet.timeout_height(),
            )
            .field(
                "ibc_packet.timeout_timestamp",
                &self.ibc_packet.timeout_timestamp(),
            )
            .field(
                "ibc_packet.data",
                &String::from_utf8_lossy(self.ibc_packet.data()),
            )
            .field("tx_signer", &self.tx_signer)
            .finish()
    }
}

async fn create_ibc_packet_from_withdrawal<S: StateRead>(
    withdrawal: Ics20Withdrawal,
    state: S,
) -> Result<IBCPacket<Unchecked>> {
    let sender = if withdrawal.use_compat_address {
        let ibc_compat_prefix = state.get_ibc_compat_prefix().await.wrap_err(
            "need to construct bech32 compatible address for IBC communication but failed reading \
             required prefix from state",
        )?;
        withdrawal
            .return_address
            .to_prefix(&ibc_compat_prefix)
            .wrap_err("failed to convert the address to the bech32 compatible prefix")?
            .to_format::<Bech32>()
            .to_string()
    } else {
        withdrawal.return_address.to_string()
    };
    let packet = FungibleTokenPacketData {
        amount: withdrawal.amount.to_string(),
        denom: withdrawal.denom.to_string(),
        sender,
        receiver: withdrawal.destination_chain_address,
        memo: withdrawal.memo,
    };

    let serialized_packet_data = serde_json::to_vec(&packet)
        .wrap_err("failed to serialize fungible token packet as JSON")?;

    Ok(IBCPacket::new(
        PortId::transfer(),
        withdrawal.source_channel,
        withdrawal.timeout_height,
        withdrawal.timeout_time,
        serialized_packet_data,
    ))
}

fn is_source(source_port: &PortId, source_channel: &ChannelId, asset: &Denom) -> bool {
    if let Denom::TracePrefixed(trace) = asset {
        !trace.has_leading_port(source_port) || !trace.has_leading_channel(source_channel)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::{
        iter,
        time::Duration,
    };

    use astria_core::{
        primitive::v1::{
            Address,
            RollupId,
            TransactionId,
        },
        protocol::transaction::v1::{
            action,
            action::{
                BridgeSudoChange,
                Ics20Withdrawal,
            },
        },
    };
    use cnidarium::StateDelta;
    use ibc_types::{
        core::{
            channel::{
                msgs::{
                    MsgChannelOpenInit,
                    MsgChannelOpenTry,
                },
                Version,
            },
            client::{
                msgs::MsgCreateClient,
                ClientId,
                ClientType,
                Height,
            },
            commitment::{
                MerklePrefix,
                MerkleProof,
            },
            connection::{
                msgs::MsgConnectionOpenInit,
                ConnectionId,
                Counterparty,
            },
        },
        lightclients::tendermint::TENDERMINT_CLIENT_TYPE,
    };
    use penumbra_ibc::{
        IbcRelay,
        IbcRelayWithHandlers,
    };
    use tendermint::Time;

    use super::{
        super::{
            test_utils::{
                address_with_prefix,
                new_create_client,
                test_asset,
                Fixture,
            },
            CheckedAction,
        },
        *,
    };
    use crate::{
        address::StateWriteExt as _,
        app::StateWriteExt as _,
        benchmark_and_test_utils::{
            assert_eyre_error,
            astria_address,
            nria,
            ASTRIA_PREFIX,
        },
        bridge::StateWriteExt as _,
        transaction::{
            StateWriteExt as _,
            TransactionContext,
        },
    };

    fn new_rollup_withdrawal() -> Ics20WithdrawalFromRollup {
        Ics20WithdrawalFromRollup {
            rollup_block_number: 2,
            rollup_withdrawal_event_id: "event-1".to_string(),
            rollup_return_address: "abc".to_string(),
            memo: "a memo".to_string(),
        }
    }

    struct Ics20WithdrawalBuilder {
        amount: u128,
        return_address: Address,
        timeout_time: u64,
        bridge_address: Option<Address>,
        rollup_withdrawal: Option<Ics20WithdrawalFromRollup>,
    }

    impl Ics20WithdrawalBuilder {
        fn new() -> Self {
            Self {
                amount: 1,
                return_address: astria_address(&[1; ADDRESS_LEN]),
                timeout_time: 100_000_000_000,
                bridge_address: None,
                rollup_withdrawal: None,
            }
        }

        fn with_amount(mut self, amount: u128) -> Self {
            self.amount = amount;
            self
        }

        fn with_return_address(mut self, return_address: Address) -> Self {
            self.return_address = return_address;
            self
        }

        fn with_timeout_time(mut self, timeout_time: u64) -> Self {
            self.timeout_time = timeout_time;
            self
        }

        fn with_bridge_address(mut self, bridge_address: Address) -> Self {
            self.bridge_address = Some(bridge_address);
            self
        }

        fn with_default_rollup_withdrawal(mut self) -> Self {
            self.rollup_withdrawal = Some(new_rollup_withdrawal());
            self
        }

        fn with_rollup_return_address<T: Into<String>>(mut self, rollup_return_address: T) -> Self {
            if self.rollup_withdrawal.is_none() {
                self.rollup_withdrawal = Some(new_rollup_withdrawal());
            }
            self.rollup_withdrawal
                .as_mut()
                .unwrap()
                .rollup_return_address = rollup_return_address.into();
            self
        }

        fn with_rollup_withdrawal_event_id<T: Into<String>>(
            mut self,
            rollup_withdrawal_event_id: T,
        ) -> Self {
            if self.rollup_withdrawal.is_none() {
                self.rollup_withdrawal = Some(new_rollup_withdrawal());
            }
            self.rollup_withdrawal
                .as_mut()
                .unwrap()
                .rollup_withdrawal_event_id = rollup_withdrawal_event_id.into();
            self
        }

        fn with_rollup_block_number(mut self, rollup_block_number: u64) -> Self {
            if self.rollup_withdrawal.is_none() {
                self.rollup_withdrawal = Some(new_rollup_withdrawal());
            }
            self.rollup_withdrawal.as_mut().unwrap().rollup_block_number = rollup_block_number;
            self
        }

        fn build(self) -> Ics20Withdrawal {
            let Self {
                amount,
                return_address,
                timeout_time,
                bridge_address,
                rollup_withdrawal,
            } = self;
            let memo = rollup_withdrawal
                .map(|rollup_withdrawal| {
                    assert!(
                        bridge_address.is_some(),
                        "setting rollup withdrawal fields has no effect if bridge address is not \
                         set"
                    );
                    serde_json::to_string(&rollup_withdrawal).unwrap()
                })
                .unwrap_or_default();
            Ics20Withdrawal {
                amount,
                denom: test_asset(),
                destination_chain_address: "test-chain".to_string(),
                return_address,
                timeout_height: Height::new(10, 1).unwrap(),
                timeout_time,
                source_channel: "channel-0".to_string().parse().unwrap(),
                fee_asset: test_asset(),
                memo,
                bridge_address,
                use_compat_address: false,
            }
        }
    }

    #[tokio::test]
    async fn should_fail_construction_if_return_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = Ics20WithdrawalBuilder::new()
            .with_return_address(address_with_prefix([50; ADDRESS_LEN], prefix))
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_timeout_time_is_zero() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new().with_timeout_time(0).build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "timeout time must be non-zero");
    }

    #[tokio::test]
    async fn should_fail_construction_if_amount_is_zero() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new().with_amount(0).build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "amount must be greater than zero");
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_address_not_base_prefixed() {
        let fixture = Fixture::new().await;

        let prefix = "different_prefix";
        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(address_with_prefix([50; ADDRESS_LEN], prefix))
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("address has prefix `{prefix}` but only `{ASTRIA_PREFIX}` is permitted"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_memo_fails_to_parse() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(astria_address(&[2; ADDRESS_LEN]))
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "failed to parse memo for ICS bound bridge withdrawal");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_return_address_is_empty() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(astria_address(&[2; ADDRESS_LEN]))
            .with_rollup_return_address("")
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup return address must be non-empty");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_return_address_is_too_long() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(astria_address(&[2; ADDRESS_LEN]))
            .with_rollup_return_address(iter::repeat_n('a', 257).collect::<String>())
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup return address must be no more than 256 bytes");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_withdrawal_event_id_is_empty() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(astria_address(&[2; ADDRESS_LEN]))
            .with_rollup_withdrawal_event_id("")
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup withdrawal event id must be non-empty");
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_withdrawal_event_id_is_too_long() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(astria_address(&[2; ADDRESS_LEN]))
            .with_rollup_withdrawal_event_id(iter::repeat_n('a', 257).collect::<String>())
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "rollup withdrawal event id must be no more than 256 bytes",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_rollup_block_number_is_zero() {
        let fixture = Fixture::new().await;

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(astria_address(&[2; ADDRESS_LEN]))
            .with_rollup_block_number(0)
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(&err, "rollup block number must be non-zero");
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_account_withdrawer_is_not_tx_signer() {
        let mut fixture = Fixture::new().await;
        let bridge_address = astria_address(&[2; ADDRESS_LEN]);
        let withdrawer_address = astria_address(&[3; ADDRESS_LEN]);
        assert_ne!(withdrawer_address.bytes(), fixture.tx_signer);
        fixture
            .state
            .put_bridge_account_withdrawer_address(&bridge_address, withdrawer_address)
            .unwrap();

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(bridge_address)
            .with_default_rollup_withdrawal()
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "transaction signer not authorized to perform ics20 bridge withdrawal",
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_account_withdrawal_event_already_processed() {
        let mut fixture = Fixture::new().await;
        let bridge_address = astria_address(&[2; ADDRESS_LEN]);
        fixture
            .state
            .put_bridge_account_withdrawer_address(&bridge_address, fixture.tx_signer)
            .unwrap();
        let event_id = "event-1".to_string();
        let block_number = 2;
        fixture
            .state
            .put_withdrawal_event_block_for_bridge_account(&bridge_address, &event_id, block_number)
            .unwrap();

        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(bridge_address)
            .with_rollup_withdrawal_event_id(&event_id)
            .build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("withdrawal event ID `{event_id}` used by block number {block_number}"),
        );
    }

    #[tokio::test]
    async fn should_fail_construction_if_bridge_account_unset_and_tx_signer_is_bridge_account() {
        let mut fixture = Fixture::new().await;
        fixture
            .state
            .put_bridge_account_rollup_id(&fixture.tx_signer, RollupId::new([99; 32]))
            .unwrap();

        let action = Ics20WithdrawalBuilder::new().build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "signer cannot be a bridge address if bridge address is not set",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_bridge_account_withdrawer_is_not_tx_signer() {
        let mut fixture = Fixture::new().await;

        // Store the tx signer as the bridge account sudo and withdrawer address.
        let bridge_address = astria_address(&[2; ADDRESS_LEN]);
        fixture
            .state
            .put_bridge_account_withdrawer_address(&bridge_address, fixture.tx_signer)
            .unwrap();
        fixture
            .state
            .put_bridge_account_sudo_address(&bridge_address, fixture.tx_signer)
            .unwrap();

        // Construct the checked ICS20 withdrawal action while the withdrawal address is still the
        // tx signer so construction succeeds.
        let action = Ics20WithdrawalBuilder::new()
            .with_bridge_address(bridge_address)
            .with_default_rollup_withdrawal()
            .build();
        let checked_action = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, &fixture.state)
            .await
            .unwrap();

        // Change the bridge account withdrawer address to one different from the tx signer address.
        let new_withdrawer_address = astria_address(&[3; ADDRESS_LEN]);
        assert_ne!(new_withdrawer_address.bytes(), fixture.tx_signer);
        let bridge_sudo_change = BridgeSudoChange {
            bridge_address,
            new_sudo_address: None,
            new_withdrawer_address: Some(new_withdrawer_address),
            fee_asset: "test".parse().unwrap(),
        };
        let checked_bridge_sudo_change = CheckedAction::new_bridge_sudo_change(
            bridge_sudo_change,
            fixture.tx_signer,
            &fixture.state,
        )
        .await
        .unwrap();
        checked_bridge_sudo_change
            .execute(&mut fixture.state)
            .await
            .unwrap();

        // Try to execute checked ICS20 withdrawal action now - should fail due to signer no longer
        // being authorized.
        let err = checked_action
            .execute(&mut fixture.state)
            .await
            .unwrap_err();
        assert_eyre_error(
            &err,
            "transaction signer not authorized to perform ics20 bridge withdrawal",
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_bridge_account_withdrawal_event_already_processed() {
        let mut fixture = Fixture::new().await;
        let bridge_address = astria_address(&[2; ADDRESS_LEN]);
        fixture.state.put_block_height(1).unwrap();
        fixture.state.put_revision_number(1).unwrap();
        fixture
            .state
            .put_bridge_account_withdrawer_address(&bridge_address, fixture.tx_signer)
            .unwrap();
        let timestamp = Time::from_unix_timestamp(1, 0).unwrap();
        fixture.state.put_block_timestamp(timestamp).unwrap();
        fixture
            .state
            .put_ibc_relayer_address(&fixture.tx_signer)
            .unwrap();
        let amount = 1314;
        fixture
            .state
            .increase_balance(&bridge_address, &test_asset(), amount)
            .await
            .unwrap();
        let create_client = new_create_client();
        let checked_action =
            CheckedAction::new_ibc_relay(create_client, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_action.execute(&mut fixture.state).await.unwrap();

        let connection_open_init = IbcRelay::ConnectionOpenInit(MsgConnectionOpenInit {
            client_id_on_a: ClientId::new(ClientType::new(TENDERMINT_CLIENT_TYPE.to_string()), 0)
                .unwrap(),
            counterparty: Counterparty {
                client_id: ClientId::new(ClientType::new(TENDERMINT_CLIENT_TYPE.to_string()), 1)
                    .unwrap(),
                connection_id: None,
                prefix: MerklePrefix::default(),
            },
            version: None,
            delay_period: Duration::from_secs(1),
            signer: String::new(),
        });
        let checked_action =
            CheckedAction::new_ibc_relay(connection_open_init, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_action.execute(&mut fixture.state).await.unwrap();

        let channel_open_init = IbcRelay::ChannelOpenInit(MsgChannelOpenInit {
            port_id_on_a: PortId::transfer(),
            connection_hops_on_a: vec![ConnectionId::new(0)],
            port_id_on_b: PortId::transfer(),
            ordering: Default::default(),
            signer: String::new(),
            version_proposal: Version::new("ics20-1".to_string()),
        });
        let checked_action =
            CheckedAction::new_ibc_relay(channel_open_init, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_action.execute(&mut fixture.state).await.unwrap();

        // https://docs.ggxchain.io/developer-documentation/ibc/ibc.html
        let channel_open_try = IbcRelay::ChannelOpenTry(MsgChannelOpenTry {
            port_id_on_b: PortId::transfer(),
            connection_hops_on_b: vec![ConnectionId::new(0)],
            port_id_on_a: PortId::transfer(),
            chan_id_on_a: ChannelId::new(0),
            version_supported_on_a: Version::new("ics20-1".to_string()),
            proof_chan_end_on_a: MerkleProof {
                proofs: vec![],
            },
            proof_height_on_a: Height::new(1, 1).unwrap(),
            ordering: Default::default(),
            signer: String::new(),
            previous_channel_id: String::new(),
            version_proposal: Version::new("ics20-1".to_string()),
        });
        let checked_action =
            CheckedAction::new_ibc_relay(channel_open_try, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        checked_action.execute(&mut fixture.state).await.unwrap();

        // Construct two identical checked ICS20 withdrawal actions.
        let action = Ics20WithdrawalBuilder::new()
            .with_amount(amount)
            .with_bridge_address(bridge_address)
            .with_default_rollup_withdrawal()
            .build();
        let checked_action_1 =
            CheckedIcs20Withdrawal::new(action.clone(), fixture.tx_signer, &fixture.state)
                .await
                .unwrap();
        let checked_action_2 =
            CheckedIcs20Withdrawal::new(action, fixture.tx_signer, &fixture.state)
                .await
                .unwrap();

        // Execute the first to write the withdrawal event to storage.
        checked_action_1.execute(&mut fixture.state).await.unwrap();

        // Try to execute the second action now - should fail due to withdrawal event already
        // having been processed.
        let err = checked_action_2
            .execute(&mut fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            &format!("withdrawal event ID `` used by block number"),
        );
    }

    #[tokio::test]
    async fn should_fail_execution_if_bridge_account_unset_and_tx_signer_is_bridge_account() {
        let mut fixture = Fixture::new().await;
        fixture
            .state
            .put_bridge_account_rollup_id(&fixture.tx_signer, RollupId::new([99; 32]))
            .unwrap();

        let action = Ics20WithdrawalBuilder::new().build();
        let err = CheckedIcs20Withdrawal::new(action, fixture.tx_signer, fixture.state)
            .await
            .unwrap_err();

        assert_eyre_error(
            &err,
            "signer cannot be a bridge address if bridge address is not set",
        );
    }

    #[test]
    fn todo() {
        todo!("when checking executes ok, ensure both versions of withdrawal address are tested");
    }
}
//     #[tokio::test]
//     async fn withdrawal_target_is_sender_if_bridge_is_not_set_and_sender_is_not_bridge() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let state = StateDelta::new(snapshot);
//
//         let denom = test_asset();
//         let from = [1u8; 20];
//         let action = action::Ics20Withdrawal {
//             amount: 1,
//             denom: denom.clone(),
//             bridge_address: None,
//             destination_chain_address: "test".to_string(),
//             return_address: astria_address(&from),
//             timeout_height: Height::new(1, 1).unwrap(),
//             timeout_time: 1,
//             source_channel: "channel-0".to_string().parse().unwrap(),
//             fee_asset: denom.clone(),
//             memo: String::new(),
//             use_compat_address: false,
//         };
//
//         assert_eq!(
//             *establish_withdrawal_target(&action, &state, &from)
//                 .await
//                 .unwrap(),
//             from
//         );
//     }
//
//     mod bridge_sender_is_rejected_because_it_is_not_a_withdrawer {
//         use super::*;
//
//         fn bridge_address() -> [u8; 20] {
//             [1; 20]
//         }
//
//         fn action() -> action::Ics20Withdrawal {
//             action::Ics20Withdrawal {
//                 amount: 1,
//                 denom: test_asset(),
//                 bridge_address: None,
//                 destination_chain_address: "test".to_string(),
//                 return_address: astria_address(&[1; 20]),
//                 timeout_height: Height::new(1, 1).unwrap(),
//                 timeout_time: 1,
//                 source_channel: "channel-0".to_string().parse().unwrap(),
//                 fee_asset: test_asset(),
//                 memo: String::new(),
//                 use_compat_address: false,
//             }
//         }
//
//         async fn run_test(action: action::Ics20Withdrawal) {
//             let storage = cnidarium::TempStorage::new().await.unwrap();
//             let snapshot = storage.latest_snapshot();
//             let mut state = StateDelta::new(snapshot);
//
//             state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//
//             // withdraw is *not* the bridge address, Ics20Withdrawal must be sent by the
// withdrawer             state
//                 .put_bridge_account_rollup_id(
//                     &bridge_address(),
//                     RollupId::from_unhashed_bytes("testrollupid"),
//                 )
//                 .unwrap();
//             state
//                 .put_bridge_account_withdrawer_address(
//                     &bridge_address(),
//                     astria_address(&[2u8; 20]),
//                 )
//                 .unwrap();
//
//             assert_eyre_error(
//                 &establish_withdrawal_target(&action, &state, &bridge_address())
//                     .await
//                     .unwrap_err(),
//                 "sender does not match bridge withdrawer address; unauthorized",
//             );
//         }
//
//         #[tokio::test]
//         async fn bridge_set() {
//             let mut action = action();
//             action.bridge_address = Some(astria_address(&bridge_address()));
//             run_test(action).await;
//         }
//     }
//
//     #[tokio::test]
//     async fn bridge_sender_is_withdrawal_target() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = StateDelta::new(snapshot);
//
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//
//         // sender the withdrawer address, so it's ok
//         let bridge_address = [1u8; 20];
//         let withdrawer_address = [2u8; 20];
//         state
//             .put_bridge_account_rollup_id(
//                 &bridge_address,
//                 RollupId::from_unhashed_bytes("testrollupid"),
//             )
//             .unwrap();
//         state
//             .put_bridge_account_withdrawer_address(&bridge_address, withdrawer_address)
//             .unwrap();
//
//         let denom = test_asset();
//         let action = action::Ics20Withdrawal {
//             amount: 1,
//             denom: denom.clone(),
//             bridge_address: Some(astria_address(&bridge_address)),
//             destination_chain_address: "test".to_string(),
//             return_address: astria_address(&bridge_address),
//             timeout_height: Height::new(1, 1).unwrap(),
//             timeout_time: 1,
//             source_channel: "channel-0".to_string().parse().unwrap(),
//             fee_asset: denom.clone(),
//             memo: String::new(),
//             use_compat_address: false,
//         };
//
//         assert_eq!(
//             *establish_withdrawal_target(&action, &state, &withdrawer_address)
//                 .await
//                 .unwrap(),
//             bridge_address,
//         );
//     }
//
//     #[tokio::test]
//     async fn bridge_is_rejected_as_withdrawal_target_because_it_has_no_withdrawer_address_set() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let state = StateDelta::new(snapshot);
//
//         // sender is not the withdrawer address, so must fail
//         let not_bridge_address = [1u8; 20];
//
//         let denom = test_asset();
//         let action = action::Ics20Withdrawal {
//             amount: 1,
//             denom: denom.clone(),
//             bridge_address: Some(astria_address(&not_bridge_address)),
//             destination_chain_address: "test".to_string(),
//             return_address: astria_address(&not_bridge_address),
//             timeout_height: Height::new(1, 1).unwrap(),
//             timeout_time: 1,
//             source_channel: "channel-0".to_string().parse().unwrap(),
//             fee_asset: denom.clone(),
//             memo: String::new(),
//             use_compat_address: false,
//         };
//
//         assert_eyre_error(
//             &establish_withdrawal_target(&action, &state, &not_bridge_address)
//                 .await
//                 .unwrap_err(),
//             "bridge address must have a withdrawer address set",
//         );
//     }
//
//     #[tokio::test]
//     async fn ics20_withdrawal_fails_if_return_address_is_not_base_prefixed() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = StateDelta::new(snapshot);
//
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: [0; 20],
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//
//         let action = action::Ics20Withdrawal {
//             amount: 1,
//             denom: nria().into(),
//             bridge_address: None,
//             destination_chain_address: "test".to_string(),
//             return_address: Address::builder()
//                 .prefix("different_prefix")
//                 .array([0; 20])
//                 .try_build()
//                 .unwrap(),
//             timeout_height: Height::new(1, 1).unwrap(),
//             timeout_time: 1,
//             source_channel: "channel-0".to_string().parse().unwrap(),
//             fee_asset: nria().into(),
//             memo: String::new(),
//             use_compat_address: false,
//         };
//
//         assert_eyre_error(
//             &action.check_and_execute(&mut state).await.unwrap_err(),
//             "failed to verify that return address has permitted base prefix",
//         );
//     }
//
//     #[tokio::test]
//     async fn ics20_withdrawal_fails_if_bridge_address_is_not_base_prefixed() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = StateDelta::new(snapshot);
//
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: [0; 20],
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//
//         let action = action::Ics20Withdrawal {
//             amount: 1,
//             denom: nria().into(),
//             bridge_address: Some(
//                 Address::builder()
//                     .prefix("different_prefix")
//                     .array([0; 20])
//                     .try_build()
//                     .unwrap(),
//             ),
//             destination_chain_address: "test".to_string(),
//             return_address: astria_address(&[0; 20]),
//             timeout_height: Height::new(1, 1).unwrap(),
//             timeout_time: 1,
//             source_channel: "channel-0".to_string().parse().unwrap(),
//             fee_asset: nria().into(),
//             memo: String::new(),
//             use_compat_address: false,
//         };
//
//         assert_eyre_error(
//             &action.check_and_execute(&mut state).await.unwrap_err(),
//             "failed to verify that bridge address address has permitted base prefix",
//         );
//     }
//
//     #[tokio::test]
//     async fn ics20_withdrawal_fails_if_bridge_address_is_set_and_memo_is_bad() {
//         let storage = cnidarium::TempStorage::new().await.unwrap();
//         let snapshot = storage.latest_snapshot();
//         let mut state = StateDelta::new(snapshot);
//
//         state.put_base_prefix(ASTRIA_PREFIX.to_string()).unwrap();
//         state.put_transaction_context(TransactionContext {
//             address_bytes: [0; 20],
//             transaction_id: TransactionId::new([0; 32]),
//             position_in_transaction: 0,
//         });
//
//         let action = action::Ics20Withdrawal {
//             amount: 1,
//             denom: nria().into(),
//             bridge_address: Some(astria_address(&[1; 20])),
//             destination_chain_address: "test".to_string(),
//             return_address: astria_address(&[0; 20]),
//             timeout_height: Height::new(1, 1).unwrap(),
//             timeout_time: 1,
//             source_channel: "channel-0".to_string().parse().unwrap(),
//             fee_asset: nria().into(),
//             memo: String::new(),
//             use_compat_address: false,
//         };
//
//         assert_eyre_error(
//             &action.check_and_execute(&mut state).await.unwrap_err(),
//             "failed to parse memo for ICS bound bridge withdrawal",
//         );
//     }
// }
