use crate::{protocol, router, Result};
use tokio::prelude::*;

/// Launch remote endpoint
#[derive(Debug, clap::Clap)]
pub(crate) struct Args;

pub(crate) async fn run(Args: Args) -> Result<()> {
    // TODO: subscriber should forward loggings to the server.
    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    let reader = protocol::RemoteCommand::new_reader(io::stdin());
    let writer = protocol::RemoteCommand::new_writer(io::stdout());

    router::spawn(protocol::ProcessKind::Local, reader, writer).await?;
    Ok(())
}
