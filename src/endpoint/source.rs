use crate::{protocol, router, Result};
use tokio::prelude::*;

pub(crate) async fn run(source: protocol::Source) -> Result<()> {
    let mut tx = router::lock().peer_tx();
    let protocol::Source { id, mut stream } = source;

    let mut buf = vec![0u8; 4096];
    loop {
        // FIXME: error handling
        let n = stream.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }

        let frame = protocol::RemoteCommand::Output(protocol::Output {
            id,
            data: buf[..n].into(),
        });
        // FIXME: error handling
        tx.send(frame).await.unwrap();
    }

    Ok(())
}
