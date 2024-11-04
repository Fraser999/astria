use std::{
    any::Any,
    sync::{
        Arc,
        Mutex,
    },
};

use async_trait::async_trait;
use clap::{
    builder::{
        BoolishValueParser,
        EnumValueParser,
    },
    ArgMatches,
    Args,
    Command,
    FromArgMatches,
    Parser,
    Subcommand,
};
use tracing::error;

use crate::{
    Action,
    OutputFormat,
    Response,
    SessionSettings,
};

/// Get or set session configuration options
#[derive(Clone, Parser, Debug)]
#[command()]
enum ConfigSubcommand {
    /// Get all session configuration options
    #[command(visible_alias = "g")]
    Get,

    /// Set session configuration options
    #[command(visible_alias = "s")]
    Set(SetConfig),
}

#[derive(Clone, Args, Debug)]
#[group(required = true, multiple = true)]
struct SetConfig {
    #[arg(
        long,
        short,
        value_name = "BOOL",
        value_parser = BoolishValueParser::new(),
        default_value = "true",
        help = "Show a 'success' or 'error' outcome after every action"
    )]
    show_outcome: Option<bool>,
    #[arg(
        long,
        short,
        value_name = "FORMAT",
        help = "Output format for responses",
        value_parser = EnumValueParser::<OutputFormat>::new(),
        default_value = "human-readable"
    )]
    output_format: Option<OutputFormat>,
}

#[derive(Clone)]
pub(crate) struct ConfigAction {
    parsed_command: Option<ConfigSubcommand>,
    settings: Arc<Mutex<SessionSettings>>,
}

impl ConfigAction {
    pub(crate) fn new(settings: Arc<Mutex<SessionSettings>>) -> Self {
        Self {
            parsed_command: None,
            settings,
        }
    }
}

#[async_trait]
impl Action for ConfigAction {
    fn name(&self) -> &'static str {
        "config"
    }

    fn display_order(&self) -> usize {
        997
    }

    fn augment_subcommand(&self, command: Command) -> Command {
        ConfigSubcommand::augment_subcommands(command)
    }

    fn set_options(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        let parsed_command = ConfigSubcommand::from_arg_matches(matches)?;
        self.parsed_command = Some(parsed_command);
        Ok(())
    }

    async fn execute(&mut self, format: OutputFormat) -> Response {
        let Some(command) = self.parsed_command.take() else {
            return Response::failure("internal error: command not set");
        };
        let mut settings = match self.settings.lock() {
            Ok(guard) => guard,
            Err(error) => {
                error!(%error, "failed to lock settings");
                return Response::failure("internal error: failed to lock settings");
            }
        };
        match command {
            ConfigSubcommand::Get => Response::success(format, "session configuration", &*settings),
            ConfigSubcommand::Set(SetConfig {
                show_outcome,
                output_format,
            }) => {
                let original_settings = *settings;

                if let Some(show_outcome) = show_outcome {
                    settings.show_outcome = show_outcome;
                }
                if let Some(output_format) = output_format {
                    settings.output_format = output_format;
                }

                if original_settings == *settings {
                    Response::success(
                        settings.output_format,
                        "session configuration unchanged",
                        &"",
                    )
                } else {
                    Response::success(settings.output_format, "session configuration updated", &"")
                }
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
