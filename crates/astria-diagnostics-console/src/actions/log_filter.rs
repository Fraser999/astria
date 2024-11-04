use std::any::Any;

use async_trait::async_trait;
use clap::{
    ArgMatches,
    Args,
    Command,
    FromArgMatches,
    Parser,
    Subcommand,
};

use crate::{
    Action,
    OutputFormat,
    Response,
};

/// Get or set the current log filter
#[derive(Clone, Parser, Debug)]
#[command()]
enum LogFilterSubcommand {
    /// Get the current log filter
    #[command(visible_alias = "g")]
    Get,

    /// Set the current log filter
    #[command(visible_alias = "s")]
    Set(SetLogFilter),
}

#[derive(Clone, Args, Debug)]
struct SetLogFilter {
    #[arg(
        value_name = "FORMAT",
        help = "Format as per https://docs.rs/env_logger/latest/env_logger/#enabling-logging"
    )]
    directive: String,
}

#[derive(Clone)]
pub(crate) struct LogFilterAction {
    parsed_command: Option<LogFilterSubcommand>,
}

impl LogFilterAction {
    pub(crate) fn new() -> Self {
        Self {
            parsed_command: None,
        }
    }
}

#[async_trait]
impl Action for LogFilterAction {
    fn name(&self) -> &'static str {
        "log-filter"
    }

    fn display_order(&self) -> usize {
        996
    }

    fn augment_subcommand(&self, command: Command) -> Command {
        LogFilterSubcommand::augment_subcommands(command)
    }

    fn set_options(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        let parsed_command = LogFilterSubcommand::from_arg_matches(matches)?;
        self.parsed_command = Some(parsed_command);
        Ok(())
    }

    async fn execute(&mut self, _format: OutputFormat) -> Response {
        Response::failure("unimplemented")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
