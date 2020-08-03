use crate::nix2io;
use nix::{
    libc,
    sys::termios::{self, Termios},
};

pub type Result<T> = std::result::Result<T, std::io::Error>;

fn enter_raw_mode() -> Result<Termios> {
    use termios::{SetArg, SpecialCharacterIndices::*};
    let fd = libc::STDIN_FILENO;

    let orig = termios::tcgetattr(fd).map_err(nix2io)?;

    let mut raw = orig.clone();
    termios::cfmakeraw(&mut raw);
    raw.control_chars[VMIN as usize] = 1;
    raw.control_chars[VTIME as usize] = 0;

    termios::tcsetattr(fd, SetArg::TCSAFLUSH, &raw).map_err(nix2io)?;

    Ok(orig)
}

fn leave_raw_mode(orig: Termios) -> Result<()> {
    use termios::SetArg;

    let fd = libc::STDIN_FILENO;
    termios::tcsetattr(fd, SetArg::TCSAFLUSH, &orig).map_err(nix2io)?;

    Ok(())
}

#[derive(Default)]
pub struct RawMode {
    orig: Option<Termios>,
}

impl RawMode {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn is_raw_mode(&self) -> bool {
        self.orig.is_some()
    }

    pub fn enter(&mut self) -> Result<bool> {
        if self.orig.is_none() {
            let orig = Some(enter_raw_mode()?);
            self.orig = orig;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn leave(&mut self) -> Result<bool> {
        if let Some(orig) = self.orig.take() {
            leave_raw_mode(orig)?;
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
