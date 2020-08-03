use crate::{ioctl, nix2io};
use nix::{
    libc,
    sys::termios::{self, Termios},
};
use std::{mem, os::unix::io::RawFd};

pub type Result<T> = std::result::Result<T, std::io::Error>;

fn enter_raw_mode(fd: RawFd) -> Result<Termios> {
    use termios::{SetArg, SpecialCharacterIndices::*};

    let orig = termios::tcgetattr(fd).map_err(nix2io)?;

    let mut raw = orig.clone();
    termios::cfmakeraw(&mut raw);
    raw.control_chars[VMIN as usize] = 1;
    raw.control_chars[VTIME as usize] = 0;

    termios::tcsetattr(fd, SetArg::TCSAFLUSH, &raw).map_err(nix2io)?;

    Ok(orig)
}

fn leave_raw_mode(fd: RawFd, orig: Termios) -> Result<()> {
    use termios::SetArg;

    termios::tcsetattr(fd, SetArg::TCSAFLUSH, &orig).map_err(nix2io)?;

    Ok(())
}

pub struct RawMode {
    fd: RawFd,
    orig: Option<Termios>,
}

impl RawMode {
    pub fn new(fd: RawFd) -> Self {
        Self { fd, orig: None }
    }

    pub fn is_raw_mode(&self) -> bool {
        self.orig.is_some()
    }

    pub fn enter(&mut self) -> Result<bool> {
        if self.orig.is_none() {
            let orig = Some(enter_raw_mode(self.fd)?);
            self.orig = orig;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn leave(&mut self) -> Result<bool> {
        if let Some(orig) = self.orig.take() {
            leave_raw_mode(self.fd, orig)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        self.leave().expect("failed to restore terminal mode");
    }
}

pub fn get_window_size(fd: RawFd) -> Result<(u16, u16)> {
    let winsz = unsafe {
        let mut winsz = mem::zeroed();
        ioctl::tiocgwinsz(fd, &mut winsz).map_err(nix2io)?;
        winsz
    };
    Ok((winsz.ws_col, winsz.ws_row))
}

pub fn set_window_size(fd: RawFd, w: u16, h: u16) -> Result<()> {
    let winsz = libc::winsize {
        ws_col: w,
        ws_row: h,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        ioctl::tiocswinsz(fd, &winsz).map_err(nix2io)?;
    };
    Ok(())
}
