use std::fmt::Display;

use serde::Serialize;
use serde_json::Value;
use tracing::warn;

use crate::OutputFormat;

/// A response to be sent back to a client as the result of executing an operation requested by the
/// client.
#[derive(Debug, Serialize)]
pub enum Response {
    /// Executing the action succeeded.
    Success {
        /// Human-readable message giving additional info and/or stating the effect.
        msg: String,
        /// The Display-formatted body.
        body: String,
    },
    /// Executing the action succeeded.
    #[serde(rename = "success")]
    SuccessJson {
        /// Human-readable message giving additional info and/or stating the effect.
        #[serde(skip_serializing_if = "String::is_empty", rename = "message")]
        msg: String,
        /// The JSON-formatted body.
        #[serde(skip_serializing_if = "Value::is_null")]
        body: Value,
    },
    /// Executing the action failed.
    Failure {
        /// Human-readable message describing the failure that occurred.
        #[serde(skip_serializing_if = "String::is_empty")]
        msg: String,
    },
}

impl Response {
    /// Constructs a new successful response.
    ///
    /// If encoding fails, a `Response::Failure` will be returned with the error included in the
    /// `msg` field.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "want to be able to pass &str here"
    )]
    pub fn success<M: ToString, B: Display + Serialize>(
        format: OutputFormat,
        outcome_msg: M,
        body: &B,
    ) -> Self {
        match format {
            OutputFormat::HumanReadable => Self::Success {
                msg: outcome_msg.to_string(),
                body: body.to_string(),
            },
            OutputFormat::Json => {
                match serde_json::to_string(body)
                    .and_then(|json_body_str| serde_json::from_str::<Value>(&json_body_str))
                {
                    Ok(mut json_body) => {
                        if json_body == Value::String(String::new()) {
                            json_body = Value::Null;
                        }
                        Self::SuccessJson {
                            msg: outcome_msg.to_string(),
                            body: json_body,
                        }
                    }
                    Err(error) => Self::failure(format!("failed to json-encode response: {error}")),
                }
            }
        }
    }

    /// Constructs a new failure response.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "want to be able to pass &str here"
    )]
    pub fn failure<M: ToString>(outcome_msg: M) -> Self {
        Self::Failure {
            msg: outcome_msg.to_string(),
        }
    }

    /// Encodes the response to a human-readable or JSON string.
    pub(crate) fn into_string(self, show_outcome: bool) -> String {
        match self {
            Response::Success {
                msg,
                body,
            } => {
                if show_outcome {
                    let mut output = String::with_capacity(
                        msg.len().saturating_add(body.len()).saturating_add(10),
                    );
                    output.push_str("success");
                    if !msg.is_empty() {
                        output.push_str(": ");
                        output.push_str(&msg);
                    }
                    if !body.is_empty() {
                        output.push_str(":\n");
                        output.push_str(&body);
                    }
                    output
                } else {
                    body
                }
            }
            Response::SuccessJson {
                msg,
                body,
            } => {
                let result = if show_outcome {
                    let response = Response::SuccessJson {
                        msg,
                        body,
                    };
                    serde_json::to_string_pretty(&response)
                } else {
                    serde_json::to_string_pretty(&body)
                };
                let output = result.unwrap_or_else(|error| {
                    warn!(%error, "error outputting json string");
                    format!(r#"{{ "internal error": "failed to output json string: {error}" }}"#)
                });
                #[expect(clippy::cmp_owned, reason = "want to use Display impl of `Value::Null")]
                if output == Value::Null.to_string() {
                    return String::new();
                }
                output
            }
            Response::Failure {
                msg,
            } => {
                if show_outcome {
                    let mut output = String::with_capacity(msg.len().saturating_add(10));
                    output.push_str("failure");
                    if !msg.is_empty() {
                        output.push_str(":\n");
                        output.push_str(&msg);
                    }
                    output.push('\n');
                    output
                } else {
                    String::from('\n')
                }
            }
        }
    }
}
