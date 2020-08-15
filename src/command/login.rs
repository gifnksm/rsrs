use super::GlobalOpts;
use crate::{
    protocol, router,
    terminal::{self, RawMode},
    Result,
};
use futures_util::stream::StreamExt as _;
use nix::{libc, unistd};
use parking_lot::Mutex;
use std::{
    env,
    ffi::{OsStr, OsString},
    panic,
    process::Stdio,
    sync::Arc,
};
use tokio::{
    io::BufReader,
    prelude::*,
    process::Command,
    signal::unix::{signal, SignalKind},
};
use tracing::{debug, info, trace, warn};

/// Starts remote login client
#[derive(Debug, clap::Clap)]
pub(super) struct Opts {
    /// Disable pseudo-terminal allocation.
    #[clap(name = "disable-pty", short = "T", overrides_with = "force-enable-pty")]
    disable_pty: bool,

    /// Force pseudo-terminal allocation.
    ///
    /// This can be used to execute arbitrary screen-based programs on a remote machine,
    /// which can be very useful, e.g. when implementing menu services.
    ///
    /// Multiple `-t` options force tty allocation, even if `ssh` has no local tty.
    #[clap(
        name = "force-enable-pty",
        short = "t",
        overrides_with = "disable-pty",
        parse(from_occurrences)
    )]
    force_enable_pty: u32,

    /// Do not execute a remote command.
    ///
    /// This is useful for just forwarding ports.
    #[clap(name = "no-remote-command", short = "N")]
    no_remote_command: bool,

    /// Commands to executed on a remote machine.
    #[clap(name = "command")]
    command: Vec<OsString>,
}

#[derive(Debug)]
enum PtyMode {
    Auto,
    Disable,
    Enable,
}

pub(super) async fn run(_: GlobalOpts, opts: Opts) -> Result<()> {
    let spawn_command = if opts.no_remote_command {
        None
    } else if opts.command.is_empty() {
        Some(protocol::SpawnCommand::LoginShell)
    } else {
        Some(protocol::SpawnCommand::Program(
            opts.command[0].clone(),
            opts.command[1..].into(),
        ))
    };

    let pty_mode = if opts.disable_pty {
        debug_assert_eq!(opts.force_enable_pty, 0);
        PtyMode::Disable
    } else if opts.force_enable_pty > 0 {
        PtyMode::Enable
    } else {
        PtyMode::Auto
    };

    let mut allocate_pty = match pty_mode {
        PtyMode::Auto => matches!(spawn_command, Some(protocol::SpawnCommand::LoginShell)),
        PtyMode::Enable => true,
        PtyMode::Disable => false,
    };

    let has_local_tty = unistd::isatty(libc::STDIN_FILENO)?;
    if opts.force_enable_pty < 2 && allocate_pty && !has_local_tty {
        warn!("Pseudo-terminal will not be allocated because stdin is not a terminal.");
        allocate_pty = false;
    }

    let exe = env::current_exe()?.canonicalize()?;
    let mut child = Command::new("ssh")
        .arg("-T")
        .arg("localhost")
        .arg(exe)
        .arg("remote")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let raw = Arc::new(Mutex::new(RawMode::new(libc::STDIN_FILENO)));
    {
        let raw = raw.clone();
        let saved_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let left = raw.lock().leave().expect("failed to restore terminal mode");
            if left {
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

    let router = router::spawn(protocol::ProcessKind::Local, reader, writer);

    let mut handler_tx = router::lock().handler_tx();

    {
        let raw = raw.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(remote_stderr).lines();
            while let Some(line) = lines.next_line().await.unwrap() {
                let is_raw_mode = raw.lock().is_raw_mode();
                if is_raw_mode {
                    eprintln!("{}\r", line);
                } else {
                    eprintln!("{}", line);
                }
            }
        });
    }

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
    if let Some(command) = spawn_command {
        if has_local_tty && allocate_pty {
            trace!("entering raw mode");
            raw.lock().enter()?;
        }

        let id = router::lock().new_id();
        let status_rx = router::lock().insert_status_notifier(id).unwrap();
        let channel_rx = router::lock().insert_channel(id).unwrap();

        {
            let mut handler_tx = handler_tx.clone();
            tokio::spawn(async move {
                let mut stream = signal(SignalKind::window_change()).unwrap();
                while let Some(()) = stream.next().await {
                    let (width, height) = terminal::get_window_size(libc::STDIN_FILENO).unwrap();
                    handler_tx
                        .send(protocol::Command::Send(protocol::RemoteCommand::Channel(
                            protocol::ChannelCommand {
                                id,
                                data: protocol::ChannelData::WindowSizeChange(width, height),
                            },
                        )))
                        .await
                        .unwrap();
                }
            });
        }

        let mut env_vars = vec![];
        let pty = if allocate_pty {
            if let Some(term) = env::var_os("TERM") {
                env_vars.push((OsStr::new("TERM").to_owned(), term));
            }

            let (width, height) = terminal::get_window_size(libc::STDIN_FILENO)?;
            Some(protocol::PtyParam { width, height })
        } else {
            None
        };

        handler_tx
            .send(protocol::Command::Sink(protocol::Sink {
                id,
                rx: channel_rx,
                stream: Box::new(io::stdout()),
                pty_name: None,
            }))
            .await?;
        handler_tx
            .send(protocol::Command::Send(protocol::RemoteCommand::Spawn(
                protocol::Spawn {
                    id,
                    command,
                    pty,
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
        raw.lock().leave()?;
        info!(status = ?remote_status.status, "remote process exited");
    } else {
        router.await?;
    }

    handler_tx
        .send(protocol::Command::Send(protocol::RemoteCommand::Exit))
        .await?;
    handler_tx
        .send(protocol::Command::Recv(protocol::RemoteCommand::Exit))
        .await?;

    let status = status.await?;
    debug!(status = ?protocol::ExitStatus::from(status), "local process exited");

    Ok(())
}
