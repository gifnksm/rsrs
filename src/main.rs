#![type_length_limit = "15524550"]

use self::prelude::*;
use clap::Clap as _;
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
    let opts = Opts::parse();

    install_tracing(opts.log_directive());
    color_eyre::install()?;

    command::run(opts).await?;

    Ok(())
}

fn install_tracing(directive: Option<&str>) {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let fmt_layer = fmt::layer().with_writer(std::io::stderr);
    let filter_layer = directive
        .map(EnvFilter::new)
        .or_else(|| EnvFilter::try_from_default_env().ok())
        .unwrap_or_else(|| EnvFilter::new("info"));
    let error_layer = ErrorLayer::default();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(error_layer)
        .init();
}
