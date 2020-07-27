use std::env;
use tokio::{prelude::*, process::Command};
use tokio_pty_command::{CommandExt as _, PtyMaster};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "1");

    // FIXME: set TERM envvar
    // FIXME: fork SHELL as a login shell (add prefix '-' to argv[0] or add --login arg)
    let shell = env::var_os("SHELL").unwrap();
    let pty_master = PtyMaster::open()?;

    let child = Command::new(shell).spawn_with_pty(&pty_master)?;

    let (child_stdout, child_stdin) = io::split(pty_master);
    let status = child;

    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    tokio::spawn(async move { rsrs::receiver(io::stdin(), child_stdin).await.unwrap() });
    tokio::spawn(async move { rsrs::sender(child_stdout, io::stdout()).await.unwrap() });

    match status.await?.code() {
        Some(code) => eprintln!("remote process exited with {}", code),
        None => eprintln!("remote process terminated by signal"),
    }

    Ok(())
}
