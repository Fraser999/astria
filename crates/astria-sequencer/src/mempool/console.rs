use std::{
    any::Any,
    fmt::{
        self,
        Display,
        Formatter,
        Write,
    },
    time::SystemTime,
};

use astria_diagnostics_console::{
    Action,
    ByteArrayFromBase64Parser,
    ByteArrayFromHexParser,
    OutputFormat,
    Response,
};
use async_trait::async_trait;
use base64::{
    prelude::BASE64_STANDARD,
    Engine as _,
};
use clap::{
    ArgMatches,
    Args,
    Command,
    FromArgMatches,
    Parser,
    Subcommand,
};
use indenter::indented;
use itertools::Itertools as _;
use serde::Serialize;
use tokio::time::Instant;

use super::{
    transactions_container::{
        TimemarkedTransaction,
        TransactionsContainer as _,
        TransactionsForAccount,
    },
    Mempool,
    RemovalReason,
};

/// Interact with the app-side mempool
#[derive(Clone, Parser, Debug)]
#[command(visible_alias = "mp")]
enum MempoolSubcommand {
    /// Show the state of the mempool
    #[command(visible_alias = "s")]
    Show(Show),

    /// Check if a specific tx exists in the mempool
    #[command(visible_alias = "c")]
    Check(TxHash),

    /// Evict a tx from the mempool
    #[command(visible_alias = "e")]
    Evict(TxHash),
}

#[derive(Clone, Args, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "bools needed for command line flags"
)]
struct Show {
    /// Exclude the pending txs from the output
    #[arg(long, default_value = "false")]
    omit_pending: bool,
    /// Exclude the parked txs from the output
    #[arg(long, default_value = "false")]
    omit_parked: bool,
    /// Exclude the txs in the cache for removal from the CometBFT mempool from the output
    #[arg(long, default_value = "false")]
    #[expect(clippy::doc_markdown, reason = "don't want backticks in help message")]
    omit_removal_cache: bool,
    /// Only include txs under the given account. This arg has no effect on the list of
    /// txs in the CometBFT removal cache.
    #[arg(
        long, short, value_name = "BASE64 STRING", value_parser = ByteArrayFromBase64Parser::<20>
    )]
    #[expect(clippy::doc_markdown, reason = "don't want backticks in help message")]
    account: Option<[u8; 20]>,
    /// Show verbose details of the state of the mempool
    #[arg(long, short, default_value = "false")]
    verbose: bool,
}

#[derive(Clone, Args, Debug)]
struct TxHash {
    /// The hex-encoded tx hash
    #[arg(value_name = "HEX STRING", value_parser = ByteArrayFromHexParser::<32>)]
    tx_hash: [u8; 32],
}

#[derive(Clone)]
pub(crate) struct MempoolAction {
    parsed_command: Option<MempoolSubcommand>,
    mempool: Mempool,
}

impl MempoolAction {
    pub(crate) fn new(mempool: Mempool) -> Self {
        Self {
            parsed_command: None,
            mempool,
        }
    }
}

#[async_trait]
impl Action for MempoolAction {
    fn name(&self) -> &'static str {
        "mempool"
    }

    fn display_order(&self) -> usize {
        0
    }

    fn augment_subcommand(&self, command: Command) -> Command {
        MempoolSubcommand::augment_subcommands(command)
    }

    fn set_options(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        let parsed_command = MempoolSubcommand::from_arg_matches(matches)?;
        self.parsed_command = Some(parsed_command);
        Ok(())
    }

    async fn execute(&mut self, format: OutputFormat) -> Response {
        let Some(command) = self.parsed_command.take() else {
            return Response::failure("internal error: command not set");
        };
        match command {
            MempoolSubcommand::Show(show) => {
                let outcome_msg = if let Some(account) = &show.account {
                    format!(
                        "mempool summary for account {}",
                        BASE64_STANDARD.encode(account)
                    )
                } else {
                    "mempool summary".to_string()
                };
                let info = Info::new(&self.mempool, show).await;
                Response::success(format, outcome_msg, &info)
            }
            MempoolSubcommand::Check(TxHash {
                tx_hash: tx_id,
            }) => {
                let outcome_msg = format!("status of tx {}", hex::encode(tx_id));
                let status = self.mempool.tx_status(&tx_id).await;
                Response::success(format, outcome_msg, &status)
            }
            MempoolSubcommand::Evict(_) => Response::failure("unimplemented"),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Serialize)]
struct Info {
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_txs: Option<PendingTxs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parked_txs: Option<ParkedTxs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    txs_for_removal: Option<RemovalCache>,
    total_txs: usize,
}

impl Info {
    async fn new(mempool: &Mempool, options: Show) -> Self {
        let pending_txs = if options.omit_pending {
            None
        } else {
            let pendings = mempool.clone_pending().await;
            let txs = if let Some(requested_account) = options.account {
                pendings
                    .txs()
                    .get(&requested_account)
                    .map(|account_pendings| {
                        TxsForAccount::from((&requested_account, account_pendings))
                    })
                    .into_iter()
                    .collect()
            } else {
                pendings.txs().iter().map(TxsForAccount::from).collect()
            };
            Some(PendingTxs {
                txs,
                tx_count: pendings.len(),
                tx_ttl: humantime::format_duration(pendings.tx_ttl()).to_string(),
            })
        };

        let parked_txs = if options.omit_parked {
            None
        } else {
            let parked = mempool.clone_parked().await;
            let txs = if let Some(requested_account) = options.account {
                parked
                    .txs()
                    .get(&requested_account)
                    .map(|account_parked| TxsForAccount::from((&requested_account, account_parked)))
                    .into_iter()
                    .collect()
            } else {
                parked.txs().iter().map(TxsForAccount::from).collect()
            };
            Some(ParkedTxs {
                txs,
                tx_count: parked.len(),
                max_parked_txs_per_account: parked.max_parked_txs_per_account(),
                max_total_parked_txs: parked.max_tx_count(),
                tx_ttl: humantime::format_duration(parked.tx_ttl()).to_string(),
            })
        };

        let txs_for_removal = if options.omit_removal_cache {
            None
        } else {
            let removal_cache = mempool.clone_removal_cache().await;
            let mut txs = removal_cache
                .iter()
                .map(|(tx_hash, reason)| TxForRemoval {
                    tx_hash: hex::encode(tx_hash),
                    reason: reason.to_string(),
                })
                .collect::<Vec<_>>();
            txs.sort();
            let tx_count = txs.len();
            Some(RemovalCache {
                txs,
                tx_count,
                max_cache_size: removal_cache.max_size(),
            })
        };

        let total_txs = mempool.len().await;

        Info {
            pending_txs,
            parked_txs,
            txs_for_removal,
            total_txs,
        }
    }
}

impl Display for Info {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(pending_txs) = &self.pending_txs {
            writeln!(f, "pending txs:")?;
            writeln!(indent(f), "{pending_txs}")?;
        }
        if let Some(parked_txs) = &self.parked_txs {
            writeln!(f, "parked txs:")?;
            writeln!(indent(f), "{parked_txs}")?;
        }
        if let Some(txs_for_removal) = &self.txs_for_removal {
            writeln!(f, "txs for removal from cometbft:")?;
            writeln!(indent(f), "{txs_for_removal}")?;
        }
        write!(f, "total txs: {}", self.total_txs)
    }
}

#[derive(Serialize)]
struct PendingTxs {
    txs: Vec<TxsForAccount>,
    tx_count: usize,
    tx_ttl: String,
}

impl Display for PendingTxs {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if !self.txs.is_empty() {
            write!(f, "{}", self.txs.iter().join("\n"))?;
        }
        writeln!(f, "total pending txs: {}", self.tx_count)?;
        write!(f, "pending tx ttl: {}", self.tx_ttl)
    }
}

#[derive(Serialize)]
#[expect(
    clippy::struct_field_names,
    reason = "field names suitable for JSON output"
)]
struct ParkedTxs {
    txs: Vec<TxsForAccount>,
    tx_count: usize,
    max_parked_txs_per_account: usize,
    max_total_parked_txs: usize,
    tx_ttl: String,
}

impl Display for ParkedTxs {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if !self.txs.is_empty() {
            write!(f, "{}", self.txs.iter().join("\n"))?;
        }
        writeln!(f, "total parked txs: {}", self.tx_count)?;
        writeln!(
            f,
            "max parked txs per account: {}",
            self.max_parked_txs_per_account
        )?;
        writeln!(f, "max total parked txs: {}", self.max_total_parked_txs)?;
        write!(f, "parked tx ttl: {}", self.tx_ttl)
    }
}

#[derive(Serialize)]
struct TxsForAccount {
    account: String,
    txs: Vec<Tx>,
}

impl Display for TxsForAccount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "account: {}", self.account)?;
        if self.txs.is_empty() {
            write!(indent(f), "no txs")?;
        } else {
            writeln!(indent(f), "{}", self.txs.iter().join("\n"))?;
        }
        Ok(())
    }
}

impl<'a, T: TransactionsForAccount> From<(&'a [u8; 20], &'a T)> for TxsForAccount {
    fn from((account, txs_for_account): (&'a [u8; 20], &'a T)) -> Self {
        TxsForAccount {
            account: BASE64_STANDARD.encode(account),
            txs: txs_for_account.txs().values().map(Tx::from).collect(),
        }
    }
}

#[derive(Serialize)]
#[expect(
    clippy::struct_field_names,
    reason = "field names suitable for JSON output"
)]
pub(super) struct Tx {
    nonce: u32,
    tx_hash: String,
    signature: String,
    verification_key: String,
    body: String,
    time_first_seen: String,
    cost: Vec<(String, u128)>,
}

impl Display for Tx {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "nonce: {}", self.nonce)?;
        writeln!(f, "tx hash: {}", self.tx_hash)?;
        writeln!(f, "signature: {}", self.signature)?;
        writeln!(f, "verification key: {}", self.verification_key)?;
        writeln!(f, "body: {}", self.body)?;
        writeln!(f, "approximate time first seen: {}", self.time_first_seen)?;
        writeln!(f, "costs:")?;
        write!(
            indent(f),
            "{}",
            self.cost
                .iter()
                .map(|(asset, amount)| format!("{asset}: {amount}"))
                .join("\n")
        )
    }
}

impl<'a> From<&'a TimemarkedTransaction> for Tx {
    fn from(ttx: &'a TimemarkedTransaction) -> Self {
        let now_instant = Instant::now();
        let now_system_time = SystemTime::now();
        let since_first_seen = now_instant.duration_since(ttx.time_first_seen());
        let first_seen = now_system_time
            .checked_sub(since_first_seen)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut cost = ttx
            .cost()
            .iter()
            .map(|(asset, cost)| (asset.to_string(), *cost))
            .collect::<Vec<_>>();
        cost.sort();
        Self {
            nonce: ttx.nonce(),
            tx_hash: hex::encode(ttx.id()),
            signature: ttx.signed_tx().signature().to_string(),
            verification_key: ttx.signed_tx().verification_key().to_string(),
            body: format!("{:?}", ttx.signed_tx().unsigned_transaction()),
            time_first_seen: humantime::format_rfc3339(first_seen).to_string(),
            cost,
        }
    }
}

#[derive(Serialize)]
struct RemovalCache {
    txs: Vec<TxForRemoval>,
    tx_count: usize,
    max_cache_size: usize,
}

impl Display for RemovalCache {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if !self.txs.is_empty() {
            write!(f, "{}", self.txs.iter().join("\n"))?;
        }
        writeln!(f, "number of txs for removal: {}", self.tx_count)?;
        write!(f, "max number of txs for removal: {}", self.max_cache_size)
    }
}

#[derive(Serialize, Eq, PartialEq, Ord, PartialOrd)]
struct TxForRemoval {
    tx_hash: String,
    reason: String,
}

impl Display for TxForRemoval {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.tx_hash, self.reason)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TxStatus {
    Absent,
    Pending(Tx),
    Parked(Tx),
    ToBeRemoved(String),
}

impl TxStatus {
    pub(super) fn pending(ttx: &TimemarkedTransaction) -> Self {
        Self::Pending(Tx::from(ttx))
    }

    pub(super) fn parked(ttx: &TimemarkedTransaction) -> Self {
        Self::Parked(Tx::from(ttx))
    }

    pub(super) fn to_be_removed(reason: &RemovalReason) -> Self {
        Self::ToBeRemoved(reason.to_string())
    }
}

impl Display for TxStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TxStatus::Absent => f.write_str("not in app mempool"),
            TxStatus::Pending(tx) => {
                writeln!(f, "in pending queue")?;
                write!(indent(f), "{tx}")
            }
            TxStatus::Parked(tx) => {
                writeln!(f, "in parked queue")?;
                write!(indent(f), "{tx}")
            }
            TxStatus::ToBeRemoved(reason) => {
                write!(f, "to be removed: {reason}")
            }
        }
    }
}

fn indent<'a, 'b>(f: &'a mut Formatter<'b>) -> indenter::Indented<'a, Formatter<'b>> {
    indented(f).with_str("    ")
}
