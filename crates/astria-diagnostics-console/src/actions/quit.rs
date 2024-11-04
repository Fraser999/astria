use std::any::Any;

use async_trait::async_trait;
use clap::{
    ArgMatches,
    Args as _,
    Command,
    FromArgMatches,
    Parser,
};

use crate::{
    Action,
    OutputFormat,
    Response,
};

/// Close the connection
#[derive(Clone, Parser, Debug)]
#[command(visible_alias = "q")]
struct QuitSubcommand {}

#[derive(Clone)]
pub(crate) struct QuitAction {
    parsed_command: Option<QuitSubcommand>,
}

impl QuitAction {
    pub(crate) fn new() -> Self {
        Self {
            parsed_command: None,
        }
    }
}

#[async_trait]
impl Action for QuitAction {
    fn name(&self) -> &'static str {
        "quit"
    }

    fn display_order(&self) -> usize {
        998
    }

    fn augment_subcommand(&self, command: Command) -> Command {
        QuitSubcommand::augment_args(command)
    }

    fn set_options(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        let parsed_command = QuitSubcommand::from_arg_matches(matches)?;
        self.parsed_command = Some(parsed_command);
        Ok(())
    }

    async fn execute(&mut self, format: OutputFormat) -> Response {
        Response::success(format, "", &"")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
