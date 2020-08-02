use parking_lot::Mutex;
use rsrs::{protocol, router, terminal::RawMode};
use std::{env, ffi::OsStr, panic, process::Stdio, sync::Arc};
use tokio::{io::BufReader, prelude::*, process::Command};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

// TODO: add tracing
// TODO: set terminal window size/termios
// TODO: argument parsing

#[tokio::main]
async fn main() -> Result<()> {
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

    let raw = Arc::new(Mutex::new(RawMode::new()?));
    {
        let raw = raw.clone();
        let saved_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let mut raw = raw.lock();
            raw.leave().expect("failed to restore terminal mode");
            eprintln!("escaped from raw mode");
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
            protocol::Spawn { id, env_vars },
        )))
        .await?;
    // FIXME: create a dedicated therad for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    handler_tx
        .send(protocol::Command::Source(protocol::Source {
            id,
            stream: Box::new(io::stdin()),
        }))
        .await?;

    let remote_status = status_rx.await?;
    raw.lock().leave()?;

    match remote_status.status {
        protocol::ExitStatus::Code(code) => eprintln!("remote process exited with {}", code),
        protocol::ExitStatus::Signal(signal) => {
            eprintln!("remote process exited with signal {}", signal)
        }
    }

    handler_tx
        .send(protocol::Command::Send(protocol::RemoteCommand::Exit))
        .await?;

    let status = status.await?;

    match status.code() {
        Some(code) => eprintln!("local process exited with {}", code),
        None => eprintln!("local process terminated by signal"),
    }

    Ok(())
}
