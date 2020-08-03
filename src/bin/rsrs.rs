use clap::derive::Clap as _;
use parking_lot::Mutex;
use rsrs::{protocol, router, terminal::RawMode};
use std::{env, ffi::OsStr, panic, process::Stdio, sync::Arc};
use tokio::{io::BufReader, prelude::*, process::Command};
use tracing::{debug, trace};
use tracing_subscriber::EnvFilter;

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, clap::Clap)]
#[clap(name = clap::crate_name!(), version = clap::crate_version!(), author = clap::crate_authors!(), about = clap::crate_description!())]
struct Args {
    /// Disable pseudo-terminal allocation.
    #[clap(name = "disable-pty", short = "T", overrides_with = "force-enable-pty")]
    disable_pty: bool,
    /// Force pseudo-terminal allocation.
    ///
    /// This can be used to execute arbitrary screen-based programs on a remote machine,
    /// which can be very useful, e.g. when implementing menu services.
    #[clap(name = "force-enable-pty", short = "t", overrides_with = "disable-pty")]
    force_enable_pty: bool,
}

// TODO: set terminal window size/termios

#[derive(Debug)]
pub enum PtyMode {
    Auto,
    Disable,
    Enable,
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("no global subscriber has been set");

    let args = Args::parse();
    trace!(args = ?args);

    let pty_mode = if args.disable_pty {
        debug_assert!(!args.force_enable_pty);
        PtyMode::Disable
    } else if args.force_enable_pty {
        PtyMode::Enable
    } else {
        PtyMode::Auto
    };

    let allocate_pty = match pty_mode {
        PtyMode::Auto => true, // TODO: fix
        PtyMode::Enable => true,
        PtyMode::Disable => false,
    };

    let mut exe = env::current_exe()?.canonicalize()?;
    let _ = exe.pop();
    exe.push("rsrs-remote");

    let mut child = Command::new("ssh")
        .arg("-T")
        .arg("localhost")
        .arg(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    trace!("entering raw mode");
    let raw = if allocate_pty {
        Some(RawMode::new()?)
    } else {
        None
    };
    let raw = Arc::new(Mutex::new(raw));
    {
        let raw = raw.clone();
        let saved_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let mut raw = raw.lock();
            if let Some(raw) = &mut *raw {
                raw.leave().expect("failed to restore terminal mode");
                trace!("escaped from raw mode");
            }
            saved_hook(info);
        }));
    }

    let remote_stdin = child.stdin.take().unwrap();
    let remote_stdout = child.stdout.take().unwrap();
    let remote_stderr = child.stderr.take().unwrap();
    let status = child;

    let reader = protocol::RemoteCommand::new_reader(remote_stdout);
    let writer = protocol::RemoteCommand::new_writer(remote_stdin);

    router::spawn(protocol::ProcessKind::Local, reader, writer);

    let mut handler_tx = router::lock().handler_tx();

    tokio::spawn(async move {
        let mut lines = BufReader::new(remote_stderr).lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            eprintln!("{}\r", line);
        }
    });

    // forward special env vars
    let mut env_vars = vec![];
    let derive_envs = &["RUST_BACKTRACE", "RUST_LOG"];
    for key in derive_envs {
        if let Some(value) = env::var_os(key) {
            env_vars.push((OsStr::new(key).to_owned(), value));
        }
    }
    handler_tx
        .send(protocol::Command::Send(protocol::RemoteCommand::SetEnv(
            protocol::SetEnv { env_vars },
        )))
        .await?;

    // Spawn command
    let id = router::lock().new_id();
    let status_rx = router::lock().insert_status_notifier(id).unwrap();
    let channel_rx = router::lock().insert_channel(id).unwrap();
    let mut env_vars = vec![];
    if let Some(term) = env::var_os("TERM") {
        env_vars.push((OsStr::new("TERM").to_owned(), term));
    }

    handler_tx
        .send(protocol::Command::Sink(protocol::Sink {
            id,
            rx: channel_rx,
            stream: Box::new(io::stdout()),
        }))
        .await?;
    handler_tx
        .send(protocol::Command::Send(protocol::RemoteCommand::Spawn(
            protocol::Spawn {
                id,
                allocate_pty,
                env_vars,
            },
        )))
        .await?;
    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    handler_tx
        .send(protocol::Command::Source(protocol::Source {
            id,
            stream: Box::new(io::stdin()),
        }))
        .await?;

    let remote_status = status_rx.await?;
    if let Some(raw) = &mut *raw.lock() {
        raw.leave()?;
    }

    debug!(status = ?remote_status.status, "remote process exited");

    handler_tx
        .send(protocol::Command::Send(protocol::RemoteCommand::Exit))
        .await?;

    let status = status.await?;
    debug!(status = ?protocol::ExitStatus::from(status), "local process exited");

    Ok(())
}
