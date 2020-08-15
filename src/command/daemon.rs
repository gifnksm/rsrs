use super::GlobalOpts;
use crate::{protocol, Result};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::FileTypeExt as _,
    process,
};
use tokio::{
    net::{UnixListener, UnixStream},
    stream::StreamExt as _,
};
use tracing::{debug, error, warn};

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
    debug!(sock_path = %sock_path.display());

    if sock_path.exists() {
        let metadata = sock_path.metadata()?;
        if !metadata.file_type().is_socket() {
            error!(sock_path = %sock_path.display(), "failed to start daemon: socket_path is already exists and it it not a socket file.");
            process::exit(1);
        }

        // Attempt to connect to the socket to determine if another server process is listening
        match UnixStream::connect(&sock_path).await {
            Ok(_stream) => {
                error!(sock_path = %sock_path.display(),
                        "failed to start daemon: another server process is running.");
                process::exit(1);
            }
            Err(e) => match e.kind() {
                io::ErrorKind::ConnectionRefused => {
                    debug!(sock_path = %sock_path.display(), error = %e,
                        "connection refused. maybe no server process is running");
                }
                _ => {
                    error!(sock_path = %sock_path.display(), error = %e,
                        "failed to start daemon: unexpected error occurred when connecting to the existing socket");
                    process::exit(1);
                }
            },
        }

        fs::remove_file(&sock_path)?;
    }

    let mut listener = UnixListener::bind(sock_path)?;
    debug!(local_addr = ?listener.local_addr()?,
            "start listening");

    if local.is_leaf {
        let mut out = io::stdout();
        out.write_all(protocol::MAGIC)?;
        out.flush()?;
    }

    while let Some(stream) = listener.next().await {
        match stream {
            Ok(stream) => debug!("connected"), // TODO
            Err(e) => {
                warn!(error = %e, "accept failed");
            }
        }
    }

    Ok(())
}
