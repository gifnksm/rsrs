use super::GlobalOpts;
use crate::{
    common,
    protocol::{
        self,
        cli::{self, Request, Response},
    },
    Result,
};
use passfd::tokio_02::FdPassingExt;
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
use futures_util::SinkExt as _;
use tracing::{debug, error, trace, warn};

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
            Ok(stream) => {
                let _ = tokio::spawn(async move { serve(stream).await.unwrap() });
            }
            Err(e) => {
                warn!(error = %e, "accept failed");
            }
        }
    }

    Ok(())
}

#[tracing::instrument(skip(stream), err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn serve(mut stream: UnixStream) -> Result<()> {
    debug!("connected");

    let (in_stream, out_stream) = stream.split();
    let mut writer = common::new_writer::<Response, _>(out_stream);
    let mut reader = common::new_reader::<Request, _>(in_stream);

    while let Some(req) = reader.next().await {
        let req = req?;
        trace!(?req, "req received");
        match req {
            Request::Open(open) => {
                let cli::Open {
                    command,
                    args,
                    has_stdin,
                    has_stdout,
                    has_stderr,
                } = open;

                let stdin = if has_stdin {
                    let fd = reader.get_ref().get_ref().as_ref().recv_fd().await;
                    Some(fd)
                } else {
                    None
                };
                let stdout = if has_stdout {
                    let fd = reader.get_ref().get_ref().as_ref().recv_fd().await?;
                    Some(fd)
                } else {
                    None
                };
                let stderr = if has_stderr {
                    let fd = reader.get_ref().get_ref().as_ref().recv_fd().await?;
                    Some(fd)
                } else {
                    None
                };

                writer.send(Response::Ok).await?;
            }
        }
    }

    Ok(())
}
