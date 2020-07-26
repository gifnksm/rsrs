use crate::{nix2io, Result};
use mio::{event::Evented, unix::EventedFd, PollOpt, Ready, Token};
use nix::{fcntl::OFlag, pty, unistd};
use std::{
    io::{self, Read, Write},
    os::unix::io::{AsRawFd, RawFd},
};

#[derive(Debug)]
pub(crate) struct PtyMaster {
    master_fd: pty::PtyMaster,
    slave_name: String,
}

impl PtyMaster {
    pub(crate) fn open() -> Result<Self> {
        let master_fd = pty::posix_openpt(
            OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        )
        .map_err(nix2io)?;
        pty::grantpt(&master_fd).map_err(nix2io)?;
        pty::unlockpt(&master_fd).map_err(nix2io)?;

        let slave_name = pty::ptsname_r(&master_fd).map_err(nix2io)?;

        Ok(Self {
            master_fd,
            slave_name,
        })
    }

    pub(crate) fn slave_name(&self) -> &str {
        &self.slave_name
    }
}

impl AsRawFd for PtyMaster {
    fn as_raw_fd(&self) -> RawFd {
        self.master_fd.as_raw_fd()
    }
}

impl Read for PtyMaster {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unistd::read(self.as_raw_fd(), buf).map_err(nix2io)
    }
}

impl Write for PtyMaster {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unistd::write(self.as_raw_fd(), buf).map_err(nix2io)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Evented for PtyMaster {
    fn register(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}
