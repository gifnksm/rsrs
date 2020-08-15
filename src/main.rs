use clap::derive::Clap as _;
use std::env;
use tracing::trace;
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

#[derive(Debug, clap::Clap)]
#[clap(name = clap::crate_name!(), version = clap::crate_version!(), author = clap::crate_authors!(), about = clap::crate_description!())]
struct Args {
    #[clap(subcommand)]
    sub_command: SubCommand,
}

#[derive(Debug, clap::Clap)]
enum SubCommand {
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Login(command::login::Args),
    #[clap(version = clap::crate_version!(), author = clap::crate_authors!())]
    Remote(command::remote::Args),
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()))
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("no global subscriber has been set");

    let args = Args::parse();
    trace!(args = ?args);

    match args.sub_command {
        SubCommand::Login(args) => command::login::run(args).await?,
        SubCommand::Remote(args) => command::remote::run(args).await?,
    }
    Ok(())
}
