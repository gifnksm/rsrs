use std::{env, process::Stdio};
use tokio::{prelude::*, process::Command};

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
        .stderr(Stdio::inherit())
        .spawn()?;

    let remote_stdin = child.stdin.take().unwrap();
    let remote_stdout = child.stdout.take().unwrap();
    let status = child;

    tokio::spawn(async move { rsrs::receiver(remote_stdout, io::stdout()).await.unwrap() });
    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    // FIXME: raw mode
    tokio::spawn(async move { rsrs::sender(io::stdin(), remote_stdin).await.unwrap() });

    match status.await?.code() {
        Some(code) => eprintln!("process exited with {}", code),
        None => eprintln!("process terminated by signal"),
    }

    Ok(())
}
