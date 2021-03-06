use crate::{
    common::{self, FdReader, FdWriter},
    daemon,
    prelude::*,
    protocol::cli::{self, Request, Response},
    Error, Result,
};
use passfd::tokio_02::FdPassingExt;
use std::{
    borrow::Cow,
    fmt::Debug,
    fs, io,
    os::unix::{
        fs::FileTypeExt as _,
        io::{AsRawFd as _, RawFd},
    },
    path::{Path, PathBuf},
};
use tokio::net::{
    unix::{ReadHalf, WriteHalf},
    UnixListener, UnixStream,
};

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(super) async fn setup(sock_path: Cow<'_, Path>) -> Result<(UnixListener, SocketGuard)> {
    let (listener, guard) = setup_socket(&sock_path)
        .await
        .wrap_err("failed to setup socket")?;

    trace!("completed");

    Ok((listener, guard))
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(super) async fn run(mut listener: UnixListener) -> Result<()> {
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

#[derive(Debug)]
pub(super) struct SocketGuard(PathBuf);

impl Drop for SocketGuard {
    fn drop(&mut self) {
        match fs::remove_file(&self.0) {
            Ok(()) => {
                debug!(sock_path = %self.0.display(), "socket file deleted");
            }
            Err(error) => {
                warn!(%error, sock_path = %self.0.display(),
                    "failed to delete socket file");
            }
        }
    }
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_socket(sock_path: impl AsRef<Path> + Debug) -> Result<(UnixListener, SocketGuard)> {
    let sock_path = sock_path.as_ref();

    if sock_path.exists() {
        let metadata = sock_path.metadata()?;
        ensure!(
            metadata.file_type().is_socket(),
            "file already exists and it is not a socket file: {}",
            sock_path.display()
        );

        // Attempt to connect to the socket to determine if another server process is listening
        match UnixStream::connect(&sock_path).await {
            Ok(_stream) => {
                bail!(
                    "another server process is running on the socket: {}",
                    sock_path.display()
                );
            }
            Err(e) => match e.kind() {
                io::ErrorKind::ConnectionRefused => {
                    debug!(sock_path = %sock_path.display(), error = %e,
                        "connection refused. maybe no server process is running");
                }
                _ => {
                    return Err(Error::new(e).wrap_err(
                        "unexpected error occurred when connecting to the existing socket",
                    ))
                }
            },
        }

        fs::remove_file(&sock_path)?;
    }

    let listener = UnixListener::bind(sock_path)?;
    let guard = SocketGuard(sock_path.to_owned());

    debug!(local_addr = ?listener.local_addr()?,
            "daemon started");

    Ok((listener, guard))
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
        let res = match req {
            Request::Open(req) => open(req, &mut reader, &mut writer).await,
        };

        // Send error response and shutdown UNIX stream
        if let Err(e) = res {
            writer.send(Response::Err(format!("{:#}", e))).await?;
            bail!(e);
        }
    }

    Ok(())
}

#[tracing::instrument(skip(req, reader, writer), err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn open(
    req: cli::Open,
    reader: &mut common::FramedRead<Request, ReadHalf<'_>>,
    writer: &mut common::FramedWrite<Response, WriteHalf<'_>>,
) -> Result<()> {
    let cli::Open { pid, command, args } = req;

    trace!("sending response");
    writer.send(Response::Ok).await?;

    let stdin = unsafe { FdWriter::from_raw_fd(recv_fd("stdin", reader, writer).await?)? };
    let stdout = unsafe { FdReader::from_raw_fd(recv_fd("stdout", reader, writer).await?)? };
    let stderr = unsafe { FdReader::from_raw_fd(recv_fd("stderr", reader, writer).await?)? };
    trace!(
        stdin = ?stdin.as_raw_fd(),
        stdout = ?stdout.as_raw_fd(),
        stderr = ?stderr.as_raw_fd(),
        "file descriptor received"
    );

    daemon::network::connect_to_leaf(stdin, stdout, stderr)
        .await
        .wrap_err("failed to connect to client")?;

    trace!("sending response to command");
    writer.send(Response::Ok).await?;

    info!(%pid, ?command, ?args, "connection opened");
    Ok(())
}

#[tracing::instrument(skip(reader, writer), err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn recv_fd(
    kind: &str,
    reader: &mut common::FramedRead<Request, ReadHalf<'_>>,
    writer: &mut common::FramedWrite<Response, WriteHalf<'_>>,
) -> Result<RawFd> {
    trace!("receiving file descriptor");
    let fd = reader
        .get_ref()
        .get_ref()
        .as_ref()
        .recv_fd()
        .await
        .wrap_err_with(|| format!("failed to receive {} fd", kind))?;
    writer.send(Response::Ok).await?;
    trace!(fd, "completed");
    Ok(fd)
}
