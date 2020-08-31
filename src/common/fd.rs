use super::nix2io;
use crate::prelude::*;
use mio::{event::Evented, unix::EventedFd, PollOpt, Ready, Token};
use nix::{libc, unistd};
use std::{
    io::prelude::*,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
};

#[derive(Debug)]
pub(crate) struct FileDesc {
    fd: RawFd,
}

impl Drop for FileDesc {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.fd) };
    }
}

impl AsRawFd for FileDesc {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl FromRawFd for FileDesc {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { fd }
    }
}

impl Read for FileDesc {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        unistd::read(self.as_raw_fd(), buf).map_err(nix2io)
    }
}

impl Write for FileDesc {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unistd::write(self.as_raw_fd(), buf).map_err(nix2io)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Evented for FileDesc {
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
