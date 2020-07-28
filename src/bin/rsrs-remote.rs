use etc_passwd::Passwd;
use std::{env, ffi::OsString, os::unix::process::CommandExt, process::Command as StdCommand};
use tokio::{prelude::*, process::Command};
use tokio_pty_command::{CommandExt as _, PtyMaster};

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> Result<()> {
    env::set_var("RUST_BACKTRACE", "1");
    // FIXME: set TERM envvar

    let shell = if let Some(passwd) = Passwd::current_user()? {
        OsString::from(passwd.shell.to_str()?)
    } else if let Some(shell) = env::var_os("SHELL") {
        shell
    } else {
        panic!("cannot get login shell for the user");
    };

    let mut arg0 = OsString::from("-");
    arg0.push(&shell);

    let pty_master = PtyMaster::open()?;

    let mut std_command = StdCommand::new(shell);
    std_command.arg0(arg0);
    let child = Command::from(std_command).spawn_with_pty(&pty_master)?;

    let (child_stdout, child_stdin) = io::split(pty_master);
    let status = child;

    // FIXME: create a dedicated thread for stdin. see https://docs.rs/tokio/0.2.22/tokio/io/fn.stdin.html
    tokio::spawn(async move { rsrs::receiver(io::stdin(), child_stdin).await.unwrap() });
    // FIXME: panic on child process exit (child_stdio.read() returns EIO)
    tokio::spawn(async move { rsrs::sender(child_stdout, io::stdout()).await.unwrap() });

    match status.await?.code() {
        Some(code) => eprintln!("remote process exited with {}", code),
        None => eprintln!("remote process terminated by signal"),
    }

    Ok(())
}
