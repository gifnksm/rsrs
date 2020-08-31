use super::FileDesc;
use crate::prelude::*;
use std::{
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::PollEvented;

#[derive(Debug)]
pub(crate) struct FdReader(PollEvented<FileDesc>);

impl AsRawFd for FdReader {
    fn as_raw_fd(&self) -> RawFd {
        self.0.get_ref().as_raw_fd()
    }
}

impl FdReader {
    pub(crate) unsafe fn from_raw_fd(fd: RawFd) -> io::Result<Self> {
        let inner = PollEvented::new(FileDesc::from_raw_fd(fd))?;
        Ok(Self(inner))
    }
}

impl AsyncRead for FdReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}
