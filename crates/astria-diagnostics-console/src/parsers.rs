use std::ffi::OsStr;

use base64::{
    prelude::{
        BASE64_STANDARD,
        BASE64_URL_SAFE,
    },
    Engine as _,
};
use clap::{
    builder::{
        StringValueParser,
        TypedValueParser,
    },
    error::{
        ContextKind,
        ContextValue,
        ErrorKind,
    },
    Arg,
    Command,
};

/// A Clap parser for parsing a base64-encoded string to a fixed-length byte array.
#[derive(Clone)]
pub struct ByteArrayFromBase64Parser<const N: usize>;

impl<const N: usize> TypedValueParser for ByteArrayFromBase64Parser<N> {
    type Value = [u8; N];

    fn parse_ref(
        &self,
        cmd: &Command,
        maybe_arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let Some(arg) = maybe_arg else {
            let mut error = clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd);
            error.insert(ContextKind::InvalidValue, ContextValue::None);
            return Err(error);
        };

        let error = |context: String| {
            let mut error = clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd);
            error.insert(
                ContextKind::InvalidArg,
                ContextValue::String(arg.to_string()),
            );
            error.insert(ContextKind::InvalidValue, ContextValue::String(context));
            error
        };

        let base64_str = StringValueParser::new().parse_ref(cmd, Some(arg), value)?;
        // Try to parse as both flavours of base64 encoding we use.
        let bytes = match BASE64_STANDARD.decode(&base64_str) {
            Ok(bytes) => bytes,
            Err(standard_error) => match BASE64_URL_SAFE.decode(&base64_str) {
                Ok(bytes) => bytes,
                Err(url_safe_error) => {
                    return Err(error(format!(
                        "failed to parse as standard base64 ({standard_error}) and as url-safe \
                         base64 ({url_safe_error})"
                    )));
                }
            },
        };
        let byte_array = Self::Value::try_from(bytes).map_err(|returned_bytes| {
            error(format!(
                "invalid array length; must be {N} bytes but got {} byte{}",
                returned_bytes.len(),
                if returned_bytes.len() == 1 { "" } else { "s" }
            ))
        })?;
        Ok(byte_array)
    }
}

/// A Clap parser for parsing a hex-encoded string to a fixed-length byte array.
#[derive(Clone)]
pub struct ByteArrayFromHexParser<const N: usize>;

impl<const N: usize> TypedValueParser for ByteArrayFromHexParser<N> {
    type Value = [u8; N];

    fn parse_ref(
        &self,
        cmd: &Command,
        maybe_arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let Some(arg) = maybe_arg else {
            let mut error = clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd);
            error.insert(ContextKind::InvalidValue, ContextValue::None);
            return Err(error);
        };

        let error = |context: String| {
            let mut error = clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd);
            error.insert(
                ContextKind::InvalidArg,
                ContextValue::String(arg.to_string()),
            );
            error.insert(ContextKind::InvalidValue, ContextValue::String(context));
            error
        };

        let hex_str = StringValueParser::new().parse_ref(cmd, Some(arg), value)?;
        let bytes = hex::decode(hex_str)
            .map_err(|hex_error| error(format!("failed to parse as hex: {hex_error}")))?;
        let byte_array = Self::Value::try_from(bytes).map_err(|returned_bytes| {
            error(format!(
                "invalid array length; must be {N} bytes but got {} byte{}",
                returned_bytes.len(),
                if returned_bytes.len() == 1 { "" } else { "s" }
            ))
        })?;
        Ok(byte_array)
    }
}
