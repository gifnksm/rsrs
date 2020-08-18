use crate::{prelude::*, Result};
use std::{borrow::Cow, path::Path};

mod command;
mod network;

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn run(sock_path: Cow<'_, Path>, is_leaf: bool) -> Result<()> {
    trace!("setting up...");

    network::setup(is_leaf)
        .map_err(|e| e.wrap_err("failed to setup network"))
        .await?;

    let (listener, _guard) = command::setup(sock_path)
        .map_err(|e| e.wrap_err("failed to setup command server"))
        .await?;

    trace!("setup completed");

    tokio::try_join!(
        command::run(listener).map_err(|e| e.wrap_err("command server end unexpectedly"))
    )?;

    Ok(())
}
