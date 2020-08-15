use clap::Clap as _;
use command::Opts;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

mod command;
mod common;
mod endpoint;
mod ioctl;
mod protocol;
mod router;
mod terminal;

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()))
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("no global subscriber has been set");

    command::run(Opts::parse()).await?;

    Ok(())
}
