use parking_lot::Mutex;
use rsrs::terminal::RawMode;
use std::{env, panic, process::Stdio, sync::Arc};
use tokio::{io::BufReader, prelude::*, process::Command};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

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

    tokio::spawn(async move { rsrs::receiver(remote_stdout, io::stdout()).await.unwrap() });
    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    tokio::spawn(async move { rsrs::sender(io::stdin(), remote_stdin).await.unwrap() });
    tokio::spawn(async move {
        let mut lines = BufReader::new(remote_stderr).lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            eprintln!("{}\r", line);
        }
    });

    let status = status.await?;
    raw.lock().leave()?;

    match status.code() {
        Some(code) => eprintln!("local process exited with {}", code),
        None => eprintln!("local process terminated by signal"),
    }

    Ok(())
}
