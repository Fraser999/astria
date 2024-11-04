use std::{
    any::Any,
    fmt::{
        self,
        Display,
        Formatter,
    },
};

use astria_diagnostics_console::{
    Action,
    OutputFormat,
    Response,
};
use async_trait::async_trait;
use clap::{
    ArgMatches,
    Args,
    Command,
    FromArgMatches,
    Parser,
    Subcommand,
};
use cnidarium::StateRead;
use futures::TryStreamExt;
use serde::Serialize;

use super::{
    Storage,
    StoredValue,
};
use crate::app::StateReadExt;

/// Read keys or values from storage
#[derive(Clone, Parser, Debug)]
#[command(visible_alias = "s")]
enum StorageSubcommand {
    /// Get from the verifiable store
    #[command(subcommand, visible_alias = "v")]
    Verifiable(StorageSubSubcommand),

    /// Get from the non-verifiable store
    #[command(subcommand, visible_alias = "nv")]
    NonVerifiable(StorageSubSubcommand),
}

#[derive(Clone, Subcommand, Debug)]
#[command()]
enum StorageSubSubcommand {
    /// List all keys under the given prefix
    #[command(visible_alias = "k")]
    ListKeys(ListKeysArgs),

    /// Get a value
    #[command(visible_alias = "v")]
    GetValue(GetValueArgs),
}

#[derive(Clone, Args, Debug)]
struct ListKeysArgs {
    /// Get the keys at the given block height. If not provided, the latest block is used
    #[arg(long, short, value_name = "INTEGER")]
    block_height: Option<u64>,

    /// The storage key prefix. If not provided, all keys are output
    #[arg(value_name = "STRING")]
    db_key_prefix: Option<String>,
}

#[derive(Clone, Args, Debug)]
struct GetValueArgs {
    /// Get the data at the given block height. If not provided, the latest block is used
    #[arg(long, short, value_name = "INTEGER")]
    block_height: Option<u64>,

    /// The storage key
    #[arg(value_name = "STRING")]
    db_key: String,
}

enum RetrievedValue {
    Hex(String),
    Parsed(String),
}

#[derive(Default, Serialize)]
struct Keys(Vec<String>);

impl Display for Keys {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.join("\n"))
    }
}

impl Extend<String> for Keys {
    fn extend<T: IntoIterator<Item = String>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

#[derive(Clone)]
pub(crate) struct StorageAction {
    parsed_command: Option<StorageSubcommand>,
    storage: Storage,
}

impl StorageAction {
    pub(crate) fn new(storage: Storage) -> Self {
        Self {
            parsed_command: None,
            storage,
        }
    }

    async fn get_value(
        &self,
        args: GetValueArgs,
        from_verifiable: bool,
        format: OutputFormat,
    ) -> Response {
        match self.do_get_value(args, from_verifiable).await {
            Ok(RetrievedValue::Hex(hex_value)) => Response::success(
                format,
                "found value, but not of type `StoredValue`, hex-encoded raw bytes follow",
                &hex_value,
            ),
            Ok(RetrievedValue::Parsed(stored_value)) => {
                Response::success(format, "found value", &stored_value)
            }
            Err(error) => Response::failure(error),
        }
    }

    async fn do_get_value(
        &self,
        args: GetValueArgs,
        from_verifiable: bool,
    ) -> Result<RetrievedValue, String> {
        let snapshot = match args.block_height {
            Some(height) => {
                let storage_version = self
                    .storage
                    .latest_snapshot()
                    .get_storage_version_by_height(height)
                    .await
                    .map_err(|error| {
                        format!(
                            "failed to get a storage snapshot at block height {height}: {error:#}"
                        )
                    })?;
                self.storage.snapshot(storage_version).ok_or_else(|| {
                    format!("storage snapshot at block height {height} not available")
                })?
            }
            None => self.storage.latest_snapshot(),
        };
        let maybe_value = if from_verifiable {
            snapshot
                .get_raw(&args.db_key)
                .await
                .map_err(|error| format!("failed to get value: {error:#}"))?
        } else {
            let mut maybe_value = snapshot
                .nonverifiable_get_raw(args.db_key.as_bytes())
                .await
                .map_err(|error| format!("failed to get value: {error:#}"))?;
            if maybe_value.is_none() {
                // Try interpreting the DB key as hex-encoded raw bytes
                if let Ok(key_bytes) = hex::decode(&args.db_key) {
                    maybe_value = snapshot
                        .nonverifiable_get_raw(&key_bytes)
                        .await
                        .map_err(|error| format!("failed to get value: {error:#}"))?;
                }
            }
            maybe_value
        };
        let Some(serialized_value) = maybe_value else {
            let msg = if let Some(height) = args.block_height {
                format!("no value under `{}` at block height {height}", args.db_key)
            } else {
                format!("no value under `{}` at latest block height", args.db_key)
            };
            return Err(msg);
        };
        Ok(StoredValue::deserialize(&serialized_value).map_or_else(
            |_| RetrievedValue::Hex(hex::encode(&serialized_value)),
            |stored_value| RetrievedValue::Parsed(format!("{stored_value:?}")),
        ))
    }

    async fn list_keys(
        &self,
        args: ListKeysArgs,
        from_verifiable: bool,
        format: OutputFormat,
    ) -> Response {
        match self.do_list_keys(&args, from_verifiable).await {
            Ok(keys) => {
                let outcome_msg = match &args.db_key_prefix {
                    Some(prefix) => format!("found following keys with prefix `{prefix}`"),
                    None => "all keys in database".to_string(),
                };
                Response::success(format, outcome_msg, &keys)
            }
            Err(error) => Response::failure(error),
        }
    }

    async fn do_list_keys(
        &self,
        args: &ListKeysArgs,
        from_verifiable: bool,
    ) -> Result<Keys, String> {
        let snapshot = match args.block_height {
            Some(height) => {
                let storage_version = self
                    .storage
                    .latest_snapshot()
                    .get_storage_version_by_height(height)
                    .await
                    .map_err(|error| {
                        format!(
                            "failed to get a storage snapshot at block height {height}: {error:#}"
                        )
                    })?;
                self.storage.snapshot(storage_version).ok_or_else(|| {
                    format!("storage snapshot at block height {height} not available")
                })?
            }
            None => self.storage.latest_snapshot(),
        };

        let prefix = args.db_key_prefix.as_deref().unwrap_or("");

        let res = if from_verifiable {
            snapshot.prefix_keys(prefix).try_collect::<Keys>().await
        } else {
            snapshot
                .nonverifiable_prefix_raw(prefix.as_bytes())
                .map_ok(|(key, _value)| {
                    String::from_utf8(key.clone()).unwrap_or_else(|_| hex::encode(key))
                })
                .try_collect::<Keys>()
                .await
        };
        match res {
            Ok(keys) if keys.0.is_empty() && prefix.is_empty() => {
                Err("no keys found in database".to_string())
            }
            Ok(keys) if keys.0.is_empty() => Err(format!("no keys found under prefix `{prefix}`",)),
            Ok(keys) => Ok(keys),
            Err(error) => Err(format!(
                "error getting keys under prefix `{prefix}`: {error:#}",
            )),
        }
    }
}

#[async_trait]
impl Action for StorageAction {
    fn name(&self) -> &'static str {
        "storage"
    }

    fn display_order(&self) -> usize {
        1
    }

    fn augment_subcommand(&self, command: Command) -> Command {
        StorageSubcommand::augment_subcommands(command)
    }

    fn set_options(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        let parsed_command = StorageSubcommand::from_arg_matches(matches)?;
        self.parsed_command = Some(parsed_command);
        Ok(())
    }

    async fn execute(&mut self, format: OutputFormat) -> Response {
        let Some(command) = self.parsed_command.take() else {
            return Response::failure("internal error: command not set");
        };
        match command {
            StorageSubcommand::Verifiable(StorageSubSubcommand::GetValue(args)) => {
                self.get_value(args, true, format).await
            }
            StorageSubcommand::Verifiable(StorageSubSubcommand::ListKeys(args)) => {
                self.list_keys(args, true, format).await
            }
            StorageSubcommand::NonVerifiable(StorageSubSubcommand::GetValue(args)) => {
                self.get_value(args, false, format).await
            }
            StorageSubcommand::NonVerifiable(StorageSubSubcommand::ListKeys(args)) => {
                self.list_keys(args, false, format).await
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
