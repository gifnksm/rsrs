use crate::{protocol, Result};
use futures_util::StreamExt;
use tokio::prelude::*;

pub(crate) async fn run(sink: protocol::Sink) -> Result<()> {
    let protocol::Sink {
        id: _,
        mut rx,
        mut stream,
    } = sink;

    while let Some(data) = rx.next().await {
        // FIXME: error handling
        match data {
            protocol::ChannelData::Output(data) => {
                stream.write_all(&data[..]).await?;
                stream.flush().await?;
            }
            protocol::ChannelData::Shutdown => {
                stream.shutdown().await?;
                break;
            }
        }
    }

    Ok(())
}
