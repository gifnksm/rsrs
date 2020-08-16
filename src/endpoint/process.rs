use crate::{
    protocol,
    router::{self, ChannelReceiver},
    terminal, Result,
};
use color_eyre::eyre::eyre;
use etc_passwd::Passwd;
use futures_util::TryFutureExt as _;
use nix::libc;
use std::{
    env,
    ffi::OsString,
    fs::OpenOptions,
    future::Future,
    os::unix::{fs::OpenOptionsExt, io::AsRawFd, process::CommandExt},
    process::{Command as StdCommand, Stdio},
};
use tokio::{prelude::*, process::Command};
use tokio_pty_command::{CommandExt as _, PtyMaster};

pub(crate) async fn run(rx: ChannelReceiver, spawn: protocol::Spawn) -> Result<()> {
    let protocol::Spawn {
        id,
        command,
        env_vars,
        pty,
    } = spawn;

    let (program, args, arg0) = match command {
        protocol::SpawnCommand::LoginShell => {
            let shell = if let Some(passwd) = Passwd::current_user()? {
                OsString::from(passwd.shell.to_str()?)
            } else if let Some(shell) = env::var_os("SHELL") {
                shell
            } else {
                panic!("cannot get login shell for the user");
            };
            let arg0 = {
                let mut arg0 = OsString::from("-");
                arg0.push(&shell);
                Some(arg0)
            };
            (shell, vec![], arg0)
        }
        protocol::SpawnCommand::Program(program, args) => (program, args, None),
    };

    let mut std_command = StdCommand::new(program);
    std_command.args(args);
    if let Some(arg0) = arg0 {
        std_command.arg0(arg0);
    }
    std_command.envs(env_vars);

    let (pty_name, status, child_stdin, child_stdout) = if let Some(param) = pty {
        let pty_master = PtyMaster::open()?;
        let slave_name = pty_master.slave_name().to_string();
        {
            let slave = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOCTTY)
                .open(&slave_name)?;
            terminal::set_window_size(slave.as_raw_fd(), param.width, param.height)?;
        }

        let child = Command::from(std_command).spawn_with_pty(&pty_master)?;
        let (child_stdout, child_stdin) = io::split(pty_master);
        (
            Some(slave_name),
            Box::new(child)
                as Box<dyn Future<Output = io::Result<std::process::ExitStatus>> + Send + Unpin>,
            Box::new(child_stdin) as Box<dyn AsyncWrite + Send + Unpin>,
            Box::new(child_stdout) as Box<dyn AsyncRead + Send + Unpin>,
        )
    } else {
        std_command.stdin(Stdio::piped());
        std_command.stdout(Stdio::piped());
        std_command.stderr(Stdio::inherit());

        let mut child = Command::from(std_command).spawn()?;
        let child_stdin = child.stdin.take().unwrap();
        let child_stdout = child.stdout.take().unwrap();

        (
            None,
            Box::new(child) as _,
            Box::new(child_stdin) as _,
            Box::new(child_stdout) as _,
        )
    };

    let mut handler_tx = router::lock().handler_tx();

    handler_tx
        .send(protocol::Command::Sink(protocol::Sink {
            id,
            rx,
            stream: child_stdin,
            pty_name,
        }))
        .map_err(|_| eyre!("send failed"))
        .await?;

    handler_tx
        .send(protocol::Command::Source(protocol::Source {
            id,
            stream: child_stdout,
        }))
        .map_err(|_| eyre!("send failed"))
        .await?;

    let code = status.await?;
    handler_tx
        .send(protocol::Command::Send(
            protocol::RemoteCommand::ProcessExit(protocol::ProcessExitStatus {
                id,
                status: code.into(),
            }),
        ))
        .map_err(|_| eyre!("send failed"))
        .await?;

    Ok(())
}
