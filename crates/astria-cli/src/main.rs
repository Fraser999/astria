use color_eyre::eyre;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .pretty()
        .with_writer(std::io::stderr)
        .with_env_filter("debug")
        .with_line_number(true)
        .with_file(true)
        .init();

    astria_cli::Cli::run().await
}
