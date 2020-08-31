#![type_length_limit = "15524550"]

use clap::Clap as _;
use color_eyre::eyre;
use command::Opts;

mod command;
mod common;
mod daemon;
mod endpoint;
mod ioctl;
mod prelude;
mod protocol;
mod router;
mod terminal;

type Error = eyre::Error;
type Result<T> = eyre::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing();
    color_eyre::install()?;

    command::run(Opts::parse()).await?;

    Ok(())
}

fn install_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let fmt_layer = fmt::layer().with_writer(std::io::stderr);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    let error_layer = ErrorLayer::default();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(error_layer)
        .init();
}
