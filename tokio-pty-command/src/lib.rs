use std::io;

mod command;
mod ioctl;
mod pty_master;
mod raw;

pub type Result<T> = std::result::Result<T, io::Error>;
pub use crate::{command::*, pty_master::*};

fn nix2io(e: nix::Error) -> io::Error {
    e.as_errno().unwrap().into()
}
