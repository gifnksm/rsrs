use super::GlobalOpts;
use crate::{protocol, Result};
use std::{ffi::OsString, fmt::Debug, process::Stdio};
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::UnixStream,
    process::{Child, Command},
    sync::watch,
};
use tracing::{debug, trace};
use tracing_futures::Instrument;

/// Launch RSRS daemon
#[derive(Debug, clap::Clap)]
pub(super) struct Opts {
    /// Command to open a remote session.
    #[clap(name = "command")]
    command: OsString,
    /// Arguments to command
    #[clap(name = "arguments")]
    args: Vec<OsString>,
}

#[tracing::instrument(skip(global, local), err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(super) async fn run(global: GlobalOpts, local: Opts) -> Result<()> {
    let sock_path = global.sock_path(false);
    debug!(sock_path = %sock_path.display());

    let mut stream = UnixStream::connect(&sock_path).await?;
    debug!(peer_cred = ?stream.peer_cred(), "connected to server");

    let mut child = Command::new(&local.command)
        .args(&local.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    launch_remote(&mut child).await?;

    Ok(())
}

async fn launch_remote(child: &mut Child) -> Result<()> {
    let mut remote_stdin = child.stdin.take().unwrap();
    let mut remote_stdout = child.stdout.take().unwrap();
    let mut remote_stderr = child.stderr.take().unwrap();

    let (tx, rx) = watch::channel(false);

    let stdin_handler = tokio::spawn({
        let rx = rx.clone();
        async move {
            forward_until_interrupted(io::stdin(), &mut remote_stdin, rx)
                .instrument(tracing::info_span!("stdin"))
                .await
                .unwrap();
            remote_stdin
        }
    });
    let stderr_handler = tokio::spawn({
        let rx = rx.clone();
        async move {
            forward_until_interrupted(&mut remote_stderr, io::stderr(), rx)
                .instrument(tracing::info_span!("stderr"))
                .await
                .unwrap();
            remote_stderr
        }
    });
    let stdout_handler = tokio::spawn(async move {
        forward_until_magic(&mut remote_stdout, io::stdout(), protocol::MAGIC)
            .instrument(tracing::info_span!("stdout"))
            .await
            .unwrap();
        remote_stdout
    });

    child.stdout = Some(stdout_handler.await?);
    let _ = tx.broadcast(true);
    child.stdin = Some(stdin_handler.await?);
    child.stderr = Some(stderr_handler.await?);

    Ok(())
}

#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn forward_until_interrupted(
    mut in_stream: impl AsyncRead + Unpin,
    mut out_stream: impl AsyncWrite + Unpin,
    mut rx: watch::Receiver<bool>,
) -> Result<()> {
    let mut buf = vec![0u8; 4096];
    loop {
        tokio::select! {
            res = in_stream.read(&mut buf) => {
                let n = res?;
                if n == 0 {
                    break;
                }
                trace!(read_len = %n, "read");
                out_stream.write_all(&buf[..n]).await?;
                out_stream.flush().await?;
            }
            res = rx.recv() => if res.unwrap_or(true) {
                break
            }
        };
    }
    trace!("forward completed: notified");
    Ok(())
}

#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn forward_until_magic(
    mut in_stream: impl AsyncRead + Unpin,
    mut out_stream: impl AsyncWrite + Unpin,
    magic: &[u8],
) -> Result<()> {
    let mut whole_buf = vec![0u8; magic.len()];
    let mut matched_len = 0;
    while matched_len < magic.len() {
        let n = in_stream.read(&mut whole_buf[matched_len..]).await?;
        if n == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }

        let read_buf = &whole_buf[0..matched_len + n];
        debug_assert!(read_buf.len() <= magic.len());
        if magic.starts_with(read_buf) {
            matched_len = read_buf.len();
            continue;
        }
        matched_len = 0;

        let mut start_idx = read_buf.len();
        for i in 1..read_buf.len() {
            if read_buf[i] != magic[0] || !magic.starts_with(&read_buf[i..]) {
                continue;
            }
            start_idx = i;
            matched_len = read_buf.len() - i;
            break;
        }

        trace!(magic_len = %magic.len(), %matched_len, read_len = %read_buf.len(),
                "read");
        out_stream.write_all(&read_buf[..start_idx]).await?;
        out_stream.flush().await?;
        if matched_len > 0 {
            whole_buf[..matched_len].copy_from_slice(&magic[..matched_len]);
        }
    }

    trace!("forward completed: magic found");
    debug_assert_eq!(matched_len, magic.len());

    Ok(())
}
