use std::env;
use tokio::{prelude::*, process::Command};
use tokio_pty_command::{CommandExt as _, PtyMaster};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    let shell = env::var_os("SHELL").unwrap();
    let pty_master = PtyMaster::open()?;

    let child = Command::new(shell).spawn_with_pty(&pty_master)?;

    let (child_stdout, child_stdin) = io::split(pty_master);
    let status = child;

    tokio::spawn(async move { rsrs::receiver(io::stdin(), child_stdin).await.unwrap() });
    tokio::spawn(async move { rsrs::sender(child_stdout, io::stdout()).await.unwrap() });

    match status.await?.code() {
        Some(code) => eprintln!("process exited with {}", code),
        None => eprintln!("process terminated by signal"),
    }

    Ok(())
}
