use rsrs::{protocol, router};
use tokio::prelude::*;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    // TODO: subscriber should forward loggings to the server.
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()))
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("no global subscriber has been set");

    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    let reader = protocol::RemoteCommand::new_reader(io::stdin());
    let writer = protocol::RemoteCommand::new_writer(io::stdout());

    router::spawn(protocol::ProcessKind::Local, reader, writer).await?;

    Ok(())
}
