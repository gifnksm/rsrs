use crate::{prelude::*, protocol, terminal, Result};
use nix::libc;
use std::{
    fs::OpenOptions,
    os::unix::{fs::OpenOptionsExt as _, io::AsRawFd},
};

pub(crate) async fn run(sink: protocol::Sink) -> Result<()> {
    let protocol::Sink {
        id: _,
        mut rx,
        mut stream,
        pty_name,
    } = sink;

    while let Some(data) = rx.next().await {
        // FIXME: error handling
        match data {
            protocol::ChannelData::Output(data) => {
                stream.write_all(&data[..]).await?;
                stream.flush().await?;
            }
            protocol::ChannelData::WindowSizeChange(width, height) => {
                if let Some(pty_name) = &pty_name {
                    let slave = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .custom_flags(libc::O_NOCTTY)
                        .open(pty_name)?;
                    terminal::set_window_size(slave.as_raw_fd(), width, height)?;
                }
            }
            protocol::ChannelData::Shutdown => {
                stream.shutdown().await?;
                break;
            }
        }
    }

    Ok(())
}
