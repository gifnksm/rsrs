use crate::{prelude::*, protocol, Result};

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn setup(is_leaf: bool) -> Result<()> {
    if is_leaf {
        setup_leaf()
            .map_err(|e| e.wrap_err("failed to setup network"))
            .await?;
    } else {
        setup_root()
            .map_err(|e| e.wrap_err("failed to setup network"))
            .await?;
    }

    trace!("completed");
    Ok(())
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_leaf() -> Result<()> {
    // send magic number to rsrs-open
    let mut out = io::stdout();
    out.write_all(protocol::MAGIC).await?;
    out.flush().await?;

    // receive packet from rsrs-daemon

    Ok(())
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_root() -> Result<()> {
    Ok(())
}
