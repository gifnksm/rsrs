use crate::{protocol, Result};
use futures_util::StreamExt;
use tokio::prelude::*;

pub(crate) async fn run(sink: protocol::Sink) -> Result<()> {
    let protocol::Sink {
        id: _,
        mut rx,
        mut stream,
    } = sink;

    while let Some(output) = rx.next().await {
        // FIXME: error handling
        stream.write_all(&output.data[..]).await?;
        stream.flush().await?;
    }

    Ok(())
}
