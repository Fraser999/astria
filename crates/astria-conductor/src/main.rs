use std::{
    error::Error,
    process::ExitCode,
};

use astria_conductor::{
    Conductor,
    Config,
    BUILD_INFO,
};
use astria_eyre::eyre::WrapErr as _;
use tracing::{
    error,
    info,
};

#[tokio::main]
async fn main() -> ExitCode {
    astria_eyre::install().expect("astria eyre hook must be the first hook installed");

    let cfg: Config = match config::get() {
        Ok(cfg) => cfg,
        Err(error) => {
            eprintln!("{BUILD_INFO}");
            let source = error.source().map(ToString::to_string).unwrap_or_default();
            eprintln!("failed to start conductor: {error}: {source}");
            // FIXME (https://github.com/astriaorg/astria/issues/368):
            //       might have to bubble up exit codes, since we might need
            //       to exit with other exit codes if something else fails
            return error.exit_code();
        }
    };

    let mut telemetry_conf = telemetry::configure()
        .set_no_otel(cfg.no_otel)
        .set_force_stdout(cfg.force_stdout)
        .set_pretty_print(cfg.pretty_print)
        .filter_directives(&cfg.log);

    if !cfg.no_metrics {
        telemetry_conf = telemetry_conf
            .metrics_addr(&cfg.metrics_http_listener_addr)
            .service_name(env!("CARGO_PKG_NAME"));
    }

    if let Err(error) = telemetry_conf
        .try_init()
        .wrap_err("failed to setup telemetry")
    {
        eprintln!("{BUILD_INFO}");
        eprintln!("initializing conductor failed:\n{e:?}");
        return ExitCode::FAILURE;
    }

    info!(
        config = serde_json::to_string(&cfg).expect("serializing to a string cannot fail"),
        "initializing conductor"
    );

    let conductor = match Conductor::new(cfg) {
        Err(error) => {
            error!(%error, "failed initializing conductor");
            return ExitCode::FAILURE;
        }
        Ok(conductor) => conductor,
    };

    conductor.run_until_stopped().await;
    info!("conductor stopped");
    ExitCode::SUCCESS
}
