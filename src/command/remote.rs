use tokio::fs::File;

use super::GlobalOpts;
use crate::{common, prelude::*, protocol, router, Error, Result};

/// Launch remote endpoint
#[derive(Debug, clap::Clap)]
pub(super) struct Opts;

pub(super) async fn run(_: GlobalOpts, _: Opts) -> Result<()> {
    // TODO: subscriber should forward loggings to the server.
    let stdin = File::open("/dev/stdin").await?;
    let stdout = File::create("/dev/stdout").await?;
    let reader = common::new_reader(stdin).err_into::<Error>();
    let writer = common::new_writer(stdout).sink_map_err(Error::from);

    router::spawn(protocol::ProcessKind::Local, reader, writer).await?;
    Ok(())
}
