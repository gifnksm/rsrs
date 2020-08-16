use super::GlobalOpts;
use crate::{common, protocol, router, Error, Result};
use futures_util::{sink::SinkExt as _, stream::TryStreamExt as _};
use tokio::prelude::*;

/// Launch remote endpoint
#[derive(Debug, clap::Clap)]
pub(super) struct Opts;

pub(super) async fn run(_: GlobalOpts, _: Opts) -> Result<()> {
    // TODO: subscriber should forward loggings to the server.
    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    let reader = common::new_reader(io::stdin()).err_into::<Error>();
    let writer = common::new_writer(io::stdout()).sink_map_err(Error::from);

    router::spawn(protocol::ProcessKind::Local, reader, writer).await?;
    Ok(())
}
