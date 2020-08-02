use crate::{protocol, router, Result};
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

        let frame = protocol::Command::Send(protocol::RemoteCommand::Output(protocol::Output {
            id,
            data: buf[..n].into(),
        }));
        tx.send(frame).await?;
    }

    Ok(())
}
