use super::GlobalOpts;
use crate::{daemon, prelude::*, Result};

/// Launch RSRS daemon
#[derive(Debug, clap::Clap)]
pub(super) struct Opts {
    #[clap(long = "as-leaf")]
    is_leaf: bool,
}

#[tracing::instrument(skip(global, local), err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(super) async fn run(global: GlobalOpts, local: Opts) -> Result<()> {
    let sock_path = global.sock_path(local.is_leaf);

    daemon::run(sock_path, local.is_leaf)
        .await
        .wrap_err("failed to launch daemon")?;

    Ok(())
}
