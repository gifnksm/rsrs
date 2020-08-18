use crate::{prelude::*, protocol, Result};
use std::{
    borrow::Cow,
    io::{self, Write},
    path::Path,
};

mod command;

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn run(sock_path: Cow<'_, Path>, is_leaf: bool) -> Result<()> {
    if is_leaf {
        let mut out = io::stdout();
        out.write_all(protocol::MAGIC)?;
        out.flush()?;
    }

    let (listener, _guard) = command::setup(sock_path)
        .map_err(|e| e.wrap_err("failed to setup command server"))
        .await?;

    tokio::try_join!(
        command::run(listener).map_err(|e| e.wrap_err("command server end unexpectedly"))
    )?;

    Ok(())
}
