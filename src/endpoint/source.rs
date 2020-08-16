use crate::{protocol, router, Result};
use color_eyre::eyre::eyre;
use futures_util::TryFutureExt as _;
use tokio::prelude::*;

pub(crate) async fn run(source: protocol::Source) -> Result<()> {
    let mut tx = router::lock().handler_tx();
    let protocol::Source { id, mut stream } = source;

    let mut buf = vec![0u8; 4096];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }

        let frame =
            protocol::Command::Send(protocol::RemoteCommand::Channel(protocol::ChannelCommand {
                id,
                data: protocol::ChannelData::Output(buf[..n].into()),
            }));
        tx.send(frame).map_err(|_| eyre!("send failed")).await?;
    }

    let frame =
        protocol::Command::Send(protocol::RemoteCommand::Channel(protocol::ChannelCommand {
            id,
            data: protocol::ChannelData::Shutdown,
        }));
    tx.send(frame).map_err(|_| eyre!("send failed")).await?;

    Ok(())
}
