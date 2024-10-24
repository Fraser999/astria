use clap::Subcommand;
use color_eyre::eyre;

mod account;
mod address;
mod balance;
mod block_height;
mod bridge_lock;
mod ics20_withdrawal;
mod init_bridge_account;
mod sign;
mod submit;
mod sudo;
mod threshold;
mod transfer;

#[derive(Debug, clap::Args)]
pub(super) struct Command {
    #[command(subcommand)]
    command: SubCommand,
}

impl Command {
    pub(super) async fn run(self) -> eyre::Result<()> {
        match self.command {
            SubCommand::Account(account) => account.run().await,
            SubCommand::Address(address) => address.run(),
            SubCommand::Balance(balance) => balance.run().await,
            SubCommand::BlockHeight(block_height) => block_height.run().await,
            SubCommand::BridgeLock(bridge_lock) => bridge_lock.run().await,
            SubCommand::InitBridgeAccount(init_bridge_account) => init_bridge_account.run().await,
            SubCommand::Sudo(sudo) => sudo.run().await,
            SubCommand::Transfer(transfer) => transfer.run().await,
            SubCommand::Threshold(threshold) => threshold.run().await,
            SubCommand::Ics20Withdrawal(ics20_withdrawal) => ics20_withdrawal.run().await,
            SubCommand::Submit(submit) => submit.run().await,
            SubCommand::Sign(sign) => sign.run(),
        }
    }
}

/// Interact with a Sequencer node
#[derive(Debug, Subcommand)]
enum SubCommand {
    /// Commands for interacting with Sequencer accounts
    Account(account::Command),
    /// Utilities for constructing and inspecting sequencer addresses
    Address(address::Command),
    /// Commands for interacting with Sequencer balances
    Balance(balance::Command),
    /// Commands for interacting with Sequencer block heights
    #[command(name = "blockheight")]
    BlockHeight(block_height::Command),
    /// Command for transferring to a bridge account
    BridgeLock(bridge_lock::Command),
    /// Command for initializing a bridge account
    InitBridgeAccount(init_bridge_account::Command),
    /// Commands requiring authority for Sequencer
    Sudo(sudo::Command),
    /// Command for sending balance between accounts
    Transfer(transfer::Command),
    /// Commands for threshold signing
    Threshold(threshold::Command),
    /// Command for withdrawing an ICS20 asset
    Ics20Withdrawal(ics20_withdrawal::Command),
    /// Submit the signed pbjson formatted Transaction.
    Submit(submit::Command),
    /// Sign a pbjson formatted TransactionBody to produce a Transaction.
    #[expect(
        clippy::doc_markdown,
        reason = "doc comments are turned into CLI help strings which currently don't use \
                  backticks"
    )]
    Sign(sign::Command),
}
