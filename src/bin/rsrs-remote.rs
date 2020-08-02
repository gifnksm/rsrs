use rsrs::{protocol, router};
use tokio::prelude::*;

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    let reader = protocol::RemoteCommand::new_reader(io::stdin());
    let writer = protocol::RemoteCommand::new_writer(io::stdout());

    router::spawn(protocol::ProcessKind::Local, reader, writer).await?;

    Ok(())
}
