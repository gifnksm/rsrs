use super::FileDesc;
use crate::prelude::*;
use std::{
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::PollEvented;

#[derive(Debug)]
pub(crate) struct FdWriter(PollEvented<FileDesc>);

impl AsRawFd for FdWriter {
    fn as_raw_fd(&self) -> RawFd {
        self.0.get_ref().as_raw_fd()
    }
}

impl FdWriter {
    pub(crate) unsafe fn from_raw_fd(fd: RawFd) -> io::Result<Self> {
        let inner = PollEvented::new(FileDesc::from_raw_fd(fd))?;
        Ok(Self(inner))
    }
}

impl AsyncWrite for FdWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
