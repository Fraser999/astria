use std::any::Any;

use async_trait::async_trait;
use clap::{
    ArgMatches,
    Command,
};

use crate::{
    OutputFormat,
    Response,
};

const HELP_TEMPLATE: &str = "{about}\n\n{all-args}";

/// A trait for a top-level command in the diagnostic console.
#[async_trait]
pub trait Action: CloneAction {
    /// The name of the action.
    ///
    /// This will override any name assigned to the `clap::Command` when it is registered.  For
    /// example, if you provide the following command:
    /// ```rust
    /// use astria_diagnostics_console::{Action, OutputFormat, Response};
    /// use clap::{ArgMatches, Args, Command, FromArgMatches, Parser, Subcommand};
    ///
    /// #[derive(Clone, clap::Parser, Debug)]
    /// #[command(name = "cmd-a")]
    /// enum MyCommand {}
    ///
    /// #[derive(Clone)]
    /// struct MyAction {
    ///     parsed_command: Option<MyCommand>,
    /// }
    ///
    /// #[async_trait::async_trait]
    /// impl Action for MyAction {
    ///     fn name(&self) -> &'static str { "my-cmd" }
    /// #   fn display_order(&self) -> usize { 1 }
    /// #   fn augment_subcommand(&self, command: Command) -> Command { unimplemented!() }
    /// #   fn set_options(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> { unimplemented!() }
    /// #   async fn execute(&mut self, format: OutputFormat) -> Response { unimplemented!() }
    /// #   fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    /// ```
    /// then the command will be registered and accessible using `my-cmd`, not `cmd-a`.
    fn name(&self) -> &'static str;

    /// The sort position of this action's `clap::Command` in the main diagnostic console's
    /// `clap::Command` help string.
    ///
    /// The lower the `display_order`, the earlier the command appears in the help string.
    ///
    /// The index must be unique among all registered actions for a given instance of a diagnostics
    /// console, and it must also be no higher than 995.
    fn display_order(&self) -> usize;

    /// As per [`Subcommand::augment_subcommands`](https://docs.rs/clap/latest/clap/trait.Subcommand.html#tymethod.augment_subcommands)
    ///
    /// Normally this will be implemented trivially by calling
    /// `MyCommand::augment_subcommands(command)` where `MyCommand` is the main `clap::Command` for
    /// this action.
    fn augment_subcommand(&self, command: Command) -> Command;

    /// Sets the action's options or configuration values by parsing `matches`.
    ///
    /// Normally this will be implemented trivially by calling
    /// `MyCommand::from_arg_matches(matches)` where `MyCommand` is the main `clap::Command` for
    /// this action, and storing the parsed `MyCommand` in an optional member variable of `self`.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails.
    fn set_options(&mut self, matches: &ArgMatches) -> Result<(), clap::Error>;

    /// Executes the configured action using the provided `format`.
    async fn execute(&mut self, format: OutputFormat) -> Response;

    /// Casts `self` to `Any`.
    ///
    /// Normally this will be trivially implemented as `fn as_any(&self) -> &dyn Any { self }`.
    fn as_any(&self) -> &dyn Any;

    /// Constructs an instance of this `Action`'s `clap::Command` to be added to the diagnostics
    /// console's main `clap::Command`.
    fn get_subcommand(&self) -> Command {
        let mut command = self.augment_subcommand(
            Command::new(self.name())
                .display_order(self.display_order())
                .help_template(HELP_TEMPLATE),
        );
        set_subcommands_help_templates(command.get_subcommands_mut());
        command
    }
}

#[expect(clippy::module_name_repetitions, reason = "this name makes sense")]
pub trait CloneAction {
    fn clone_action(&self) -> Box<dyn Action + Send>;
}

impl<T: Action + Send + Clone + 'static> CloneAction for T {
    fn clone_action(&self) -> Box<dyn Action + Send> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Action + Send> {
    fn clone(&self) -> Self {
        self.clone_action()
    }
}

fn set_subcommands_help_templates<'a, I: Iterator<Item = &'a mut Command>>(subcommands_iter: I) {
    for subcommand in subcommands_iter {
        *subcommand = subcommand.clone().help_template(HELP_TEMPLATE);
        set_subcommands_help_templates(subcommand.get_subcommands_mut());
    }
}
