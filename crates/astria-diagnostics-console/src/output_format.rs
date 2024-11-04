use std::fmt::{
    self,
    Display,
    Formatter,
};

use clap::{
    builder::PossibleValue,
    ValueEnum,
};
use serde::Serialize;

/// Format for responses to the client.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Default)]
pub enum OutputFormat {
    /// Human-readable format.
    ///
    /// Utilizes the `Display` implementation of types.
    #[default]
    HumanReadable,
    /// JSON, pretty-printed.
    Json,
}

impl ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::HumanReadable, Self::Json]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            OutputFormat::HumanReadable => Some(PossibleValue::new("human-readable")),
            OutputFormat::Json => Some(PossibleValue::new("json")),
        }
    }
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::HumanReadable => f.write_str("human-readable"),
            OutputFormat::Json => f.write_str("json"),
        }
    }
}
