use std::{env, process::Stdio};
use tokio::{prelude::*, process::Command};

type Result<T> = std::io::Result<T>;

#[tokio::main]
async fn main() -> Result<()> {
    let shell = env::var_os("SHELL").unwrap();

    let mut child = Command::new(shell)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let child_stdin = child.stdin.take().unwrap();
    let child_stdout = child.stdout.take().unwrap();
    let status = child;

    tokio::spawn(async move { rsrs::receiver(io::stdin(), child_stdin).await.unwrap() });
    tokio::spawn(async move { rsrs::sender(child_stdout, io::stdout()).await.unwrap() });

    match status.await?.code() {
        Some(code) => eprintln!("process exited with {}", code),
        None => eprintln!("process terminated by signal"),
    }

    Ok(())
}
