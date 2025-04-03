#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::arithmetic_side_effects,
    reason = "casts between f64 and u64 will involve values where these lints are not a problem"
)]

use std::time::Duration;

use astria_eyre::eyre::{
    ensure,
    eyre,
    Result,
    WrapErr,
};
use isahc::AsyncReadResponseExt;
use jiff::{
    Span,
    SpanTotal,
    Timestamp,
    Unit,
};
use serde_json::Value;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Duration until activation point
    #[arg(
        long,
        short = 'd',
        value_name = "DURATION",
        value_parser = clap::value_parser!(Span)
    )]
    duration: Span,

    /// The URL of the Sequencer node
    #[arg(long, short = 'u', value_name = "URL")]
    sequencer_url: String,

    /// Print verbose output
    #[arg(long, short = 'v')]
    verbose: bool,
}

impl Args {
    async fn get_current_height(&self) -> Result<(u64, Timestamp)> {
        let blocking_getter = async {
            isahc::get_async(format!("{}/block", self.sequencer_url))
                .await
                .wrap_err("failed to get latest block")?
                .text()
                .await
                .wrap_err("failed to parse block response as UTF-8 string")
        };

        let response = tokio::time::timeout(Duration::from_secs(5), blocking_getter)
            .await
            .wrap_err("timed out fetching block")??
            .trim()
            .to_string();
        let json_rpc_response: Value = serde_json::from_str(&response)
            .wrap_err_with(|| format!("failed to parse block response `{response}` as json"))?;
        let header = json_rpc_response
            .get("result")
            .and_then(|value| value.get("block"))
            .and_then(|value| value.get("header"))
            .ok_or_else(|| {
                eyre!("expected block response `{response}` to have field `result.block.header`")
            })?;
        let height_str = header
            .get("height")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("expected header `{header}` to have string field `height`"))?;
        let height: u64 = height_str
            .parse()
            .wrap_err_with(|| format!("expected height `{height_str}` to convert to `u64`"))?;
        let time_str = header
            .get("time")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("expected header `{header}` to have string field `time`"))?;
        let timestamp: Timestamp = time_str
            .parse()
            .wrap_err_with(|| format!("expected time `{time_str}` to convert to `Timestamp`"))?;

        Ok((height, timestamp))
    }

    async fn get_timestamp_at_height(&self, height: u64) -> Result<Timestamp> {
        let blocking_getter = async {
            isahc::get_async(format!("{}/block?height={height}", self.sequencer_url))
                .await
                .wrap_err_with(|| format!("failed to get block at height {height}"))?
                .text()
                .await
                .wrap_err("failed to parse block response as UTF-8 string")
        };

        let response = tokio::time::timeout(Duration::from_secs(5), blocking_getter)
            .await
            .wrap_err_with(|| format!("timed out fetching block at height {height}"))??
            .trim()
            .to_string();
        let json_rpc_response: Value = serde_json::from_str(&response)
            .wrap_err_with(|| format!("failed to parse block response `{response}` as json"))?;
        let time_str = json_rpc_response
            .get("result")
            .and_then(|value| value.get("block"))
            .and_then(|value| value.get("header"))
            .and_then(|value| value.get("time"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                eyre!(
                    "expected block response `{response}` to have string field \
                     `result.block.header.time`"
                )
            })?;
        time_str
            .parse()
            .wrap_err_with(|| format!("expected time `{time_str}` to convert to `Timestamp`"))
    }

    async fn get_network_name(&self) -> Result<String> {
        let blocking_getter = async {
            isahc::get_async(format!("{}/status", self.sequencer_url))
                .await
                .wrap_err("failed to get status")?
                .text()
                .await
                .wrap_err("failed to parse status response as UTF-8 string")
        };

        let response = tokio::time::timeout(Duration::from_secs(5), blocking_getter)
            .await
            .wrap_err("timed out fetching status")??
            .trim()
            .to_string();
        let json_rpc_response: Value = serde_json::from_str(&response)
            .wrap_err_with(|| format!("failed to parse status response `{response}` as json"))?;
        Ok(json_rpc_response
            .get("result")
            .and_then(|value| value.get("node_info"))
            .and_then(|value| value.get("network"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                eyre!(
                    "expected status response `{response}` to have string field \
                     `result.node_info.network`"
                )
            })?
            .to_string())
    }

    fn calculate_height(
        &self,
        current_height: u64,
        height_diff: u64,
        timestamp_diff: Span,
    ) -> Result<u64> {
        let duration_ms = self
            .duration
            .total(SpanTotal::from(Unit::Millisecond).days_are_24_hours())
            .wrap_err("failed to get duration total milliseconds")?;
        let time_diff_ms = timestamp_diff
            .total(Unit::Millisecond)
            .wrap_err("failed to get time difference total milliseconds")?;

        let calculated_height_diff =
            (duration_ms * height_diff as f64 / time_diff_ms).ceil() as u64;
        let calculated_height = current_height + calculated_height_diff;
        if self.verbose {
            println!("calculated height difference: {calculated_height_diff}");
        }
        Ok(calculated_height)
    }
}

/// Calculates the activation height.
///
/// # Errors
///
/// Returns an error if stuff goes wrong.
pub async fn run(mut args: Args) -> Result<()> {
    args.sequencer_url = args.sequencer_url.trim_end_matches('/').to_string();
    let (current_height, current_timestamp) = args.get_current_height().await?;
    ensure!(
        current_height > 1,
        "need current height to be greater than 1"
    );
    let height_diff = std::cmp::min(1000, current_height);
    let old_timestamp = args
        .get_timestamp_at_height(current_height - height_diff)
        .await?;
    let network_name = args.get_network_name().await?;
    if args.verbose {
        println!("current height on `{network_name}`: {current_height}");
    }
    let calculated_height = args.calculate_height(
        current_height,
        height_diff,
        current_timestamp - old_timestamp,
    )?;
    if args.verbose {
        println!("calculated activation height on `{network_name}`: {calculated_height}");
        let duration_ms =
            args.duration
                .total(SpanTotal::from(Unit::Millisecond).days_are_24_hours())
                .wrap_err("failed to get duration total milliseconds")? as u64;
        println!(
            "calculated activation instant on `{network_name}`: {}",
            current_timestamp + Duration::from_millis(duration_ms)
        );
    } else {
        print!("{calculated_height}");
    }
    Ok(())
}
