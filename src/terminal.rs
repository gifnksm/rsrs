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

pub struct RawMode {
    orig: Option<Termios>,
}

impl RawMode {
    pub fn new() -> Result<Self> {
        let orig = Some(enter_raw_mode()?);
        Ok(RawMode { orig })
    }

    pub fn leave(&mut self) -> Result<()> {
        if let Some(orig) = self.orig.take() {
            leave_raw_mode(orig)?;
        }
        Ok(())
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        self.leave().expect("failed to restore terminal mode");
    }
}
