use crate::{
    protocol,
    router::{self, Receiver},
    Result,
};
use etc_passwd::Passwd;
use std::{env, ffi::OsString, os::unix::process::CommandExt, process::Command as StdCommand};
use tokio::{prelude::*, process::Command};
use tokio_pty_command::{CommandExt as _, PtyMaster};

pub(crate) async fn run(rx: Receiver, spawn: protocol::Spawn) -> Result<()> {
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

    let mut handler_tx = router::lock().handler_tx();

    handler_tx
        .send(protocol::Command::Local(protocol::LocalCommand::Sink(
            protocol::Sink {
                id: spawn.id,
                rx,
                stream: Box::new(child_stdin),
            },
        )))
        .await
        .unwrap();

    handler_tx
        .send(protocol::Command::Local(protocol::LocalCommand::Source(
            protocol::Source {
                id: spawn.id,
                stream: Box::new(child_stdout),
            },
        )))
        .await
        .unwrap();

    match status.await?.code() {
        Some(code) => eprintln!("remote process exited with {}", code),
        None => eprintln!("remote process terminated by signal"),
    }

    Ok(())
}
