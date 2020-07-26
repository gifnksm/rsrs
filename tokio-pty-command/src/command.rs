use crate::{ioctl, nix2io, PtyMaster, Result};
use nix::{libc, unistd};
use std::{
    fs::OpenOptions,
    future::Future,
    os::unix::fs::OpenOptionsExt,
    pin::Pin,
    process::ExitStatus,
    task::{Context, Poll},
};
use tokio::process;

pub trait CommandExt {
    fn spawn_with_pty(&mut self, pty_master: &PtyMaster) -> Result<Child>;
}

impl CommandExt for process::Command {
    fn spawn_with_pty(&mut self, pty_master: &PtyMaster) -> Result<Child> {
        let slave = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY)
            .open(pty_master.slave_name())?;

        self.stdin(slave.try_clone().unwrap());
        self.stdout(slave.try_clone().unwrap());
        self.stderr(slave);

        unsafe {
            self.pre_exec(move || {
                let _pid = unistd::setsid().map_err(nix2io)?;
                ioctl::tiocsctty(0, 1).map_err(nix2io)?;
                Ok(())
            });
        }

        Ok(Child(self.spawn()?))
    }
}

#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Child(process::Child);

impl Child {
    pub fn id(&self) -> u32 {
        self.0.id()
    }

    pub fn kill(&mut self) -> Result<()> {
        self.0.kill()
    }
}

impl Future for Child {
    type Output = Result<ExitStatus>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}
