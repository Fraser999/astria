//! This crate provides a general-purpose diagnostics console

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub use action::Action;
pub(crate) use client_session::{
    ClientSession,
    SessionSettings,
};
pub use config::Config;
pub use diagnostics_console::{
    DiagnosticsConsole,
    NextStep,
};
pub use error::{
    InitializationError,
    RegistrationError,
};
pub use output_format::OutputFormat;
pub use parsers::{
    ByteArrayFromBase64Parser,
    ByteArrayFromHexParser,
};
pub use response::Response;

mod action;
pub(crate) mod actions;
mod client_session;
mod config;
mod diagnostics_console;
mod error;
mod output_format;
mod parsers;
mod response;
