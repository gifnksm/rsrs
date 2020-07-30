use crate::{
    protocol,
    router::{self, Receiver},
    Result,
};
use etc_passwd::Passwd;
use futures_util::StreamExt;
use std::{env, ffi::OsString, os::unix::process::CommandExt, process::Command as StdCommand};
use tokio::{prelude::*, process::Command};
use tokio_pty_command::{CommandExt as _, PtyMaster};

// TODO: abstract as endpoint

pub(crate) async fn spawn_process(rx: Receiver, spawn: protocol::Spawn) -> Result<()> {
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

    tokio::spawn(async move {
        let mut rx = rx;
        let mut child_stdin = child_stdin;
        while let Some(output) = rx.next().await {
            // FIXME: error handling
            child_stdin.write_all(&output.data[..]).await.unwrap();
            child_stdin.flush().await.unwrap();
        }
    });

    let tx = router::lock().sender();
    tokio::spawn(async move {
        let mut tx = tx;
        let mut child_stdout = child_stdout;
        let mut buf = vec![0u8; 4096];
        loop {
            // FIXME: error handling
            let n = child_stdout.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }

            let frame = protocol::RemoteCommand::Output(protocol::Output {
                id: spawn.id,
                data: buf[..n].into(),
            });
            // FIXME: error handling
            tx.send(frame).await.unwrap();
        }
    });

    match status.await?.code() {
        Some(code) => eprintln!("remote process exited with {}", code),
        None => eprintln!("remote process terminated by signal"),
    }

    Ok(())
}
