use std::ffi::OsStr;

use astria_core::{
    crypto::SigningKey,
    primitive::v1::Address,
    protocol::transaction::v1alpha1::{
        Action,
        UnsignedTransaction,
    },
};
use astria_sequencer_client::{
    tendermint_rpc::endpoint::tx::Response,
    HttpClient,
    SequencerClientExt as _,
};
use clap::{
    error::{
        ContextKind,
        ContextValue,
        ErrorKind,
    },
    Arg,
    Command,
    Error,
};
use color_eyre::eyre::{
    self,
    ensure,
    WrapErr as _,
};

pub(crate) async fn submit_transaction(
    sequencer_url: &str,
    chain_id: String,
    prefix: &str,
    signing_key: &SigningKey,
    action: Action,
) -> eyre::Result<Response> {
    let sequencer_client =
        HttpClient::new(sequencer_url).wrap_err("failed constructing http sequencer client")?;

    let from_address = address_from_signing_key(signing_key, prefix)?;
    println!("sending tx from address: {from_address}");

    let nonce_res = sequencer_client
        .get_latest_nonce(from_address)
        .await
        .wrap_err("failed to get nonce")?;

    let tx = UnsignedTransaction::builder()
        .nonce(nonce_res.nonce)
        .chain_id(chain_id)
        .actions(vec![action])
        .try_build()
        .wrap_err("failed to construct a transaction")?
        .into_signed(signing_key);
    let res = sequencer_client
        .submit_transaction_sync(tx)
        .await
        .wrap_err("failed to submit transaction")?;

    let tx_response = sequencer_client.wait_for_tx_inclusion(res.hash).await;

    ensure!(res.code.is_ok(), "failed to check tx: {}", res.log);

    ensure!(
        tx_response.tx_result.code.is_ok(),
        "failed to execute tx: {}",
        tx_response.tx_result.log
    );
    Ok(tx_response)
}

#[derive(Clone)]
pub(crate) struct SigningKeyParser;

impl clap::builder::TypedValueParser for SigningKeyParser {
    type Value = SigningKey;

    fn parse_ref(
        &self,
        cmd: &Command,
        maybe_arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, Error> {
        let Some(arg) = maybe_arg else {
            let mut error = Error::new(ErrorKind::ValueValidation).with_cmd(cmd);
            error.insert(ContextKind::InvalidValue, ContextValue::None);
            return Err(error);
        };

        let error = |context: String| {
            let mut e = Error::new(ErrorKind::ValueValidation).with_cmd(cmd);
            e.insert(
                ContextKind::InvalidArg,
                ContextValue::String(arg.to_string()),
            );
            e.insert(ContextKind::InvalidValue, ContextValue::String(context));
            e
        };

        let hex_str = clap::builder::StringValueParser::new().parse_ref(cmd, Some(arg), value)?;
        let bytes = hex::decode(hex_str)
            .map_err(|hex_error| error(format!("failed to parse as hex: {hex_error}")))?;
        let byte_array = <[u8; 32]>::try_from(bytes).map_err(|returned_bytes| {
            error(format!(
                "invalid signing key length; must be 32 bytes but got {} byte{}",
                returned_bytes.len(),
                if returned_bytes.len() == 1 { "" } else { "s" }
            ))
        })?;
        Ok(SigningKey::from(byte_array))
    }
}

pub(crate) fn address_from_signing_key(
    signing_key: &SigningKey,
    prefix: &str,
) -> eyre::Result<Address> {
    // Build the address using the public key from the signing key
    let from_address = Address::builder()
        .array(*signing_key.verification_key().address_bytes())
        .prefix(prefix)
        .try_build()
        .wrap_err("failed constructing a valid from address from the provided prefix")?;

    // Return the generated address
    Ok(from_address)
}
