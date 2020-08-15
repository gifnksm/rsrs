use crate::router;
use std::{ffi::OsString, os::unix::process::ExitStatusExt as _};
use tokio::prelude::*;

pub(crate) const MAGIC: &[u8] = b"\0RSRS\0magic\0number\0";

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum ProcessKind {
    Local,
    Remote,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct Id(ProcessKind, usize);

impl Id {
    pub(crate) fn new(kind: ProcessKind, id: usize) -> Self {
        Self(kind, id)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum RemoteCommand {
    SetEnv(SetEnv),
    Spawn(Spawn),
    Channel(ChannelCommand),
    ProcessExit(ProcessExitStatus),
    Exit,
}

#[derive(Debug)]
pub(crate) enum Command {
    Recv(RemoteCommand),
    Send(RemoteCommand),
    Source(Source),
    Sink(Sink),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct SetEnv {
    pub(crate) env_vars: Vec<(OsString, OsString)>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum SpawnCommand {
    LoginShell,
    Program(OsString, Vec<OsString>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Spawn {
    pub(crate) id: Id,
    pub(crate) command: SpawnCommand,
    pub(crate) env_vars: Vec<(OsString, OsString)>,
    pub(crate) pty: Option<PtyParam>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct PtyParam {
    pub(crate) width: u16,
    pub(crate) height: u16,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ChannelCommand {
    pub(crate) id: Id,
    pub(crate) data: ChannelData,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ChannelData {
    Output(Vec<u8>),
    WindowSizeChange(u16, u16),
    Shutdown,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ProcessExitStatus {
    pub(crate) id: Id,
    pub(crate) status: ExitStatus,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ExitStatus {
    Code(i32),
    Signal(i32),
}

impl From<std::process::ExitStatus> for ExitStatus {
    fn from(status: std::process::ExitStatus) -> Self {
        if let Some(code) = status.code() {
            Self::Code(code)
        } else if let Some(signal) = status.signal() {
            Self::Signal(signal)
        } else {
            panic!("invalid exit status")
        }
    }
}

#[derive(custom_debug::Debug)]
pub(crate) struct Source {
    pub(crate) id: Id,
    #[debug(skip)]
    pub(crate) stream: Box<dyn AsyncRead + Send + Unpin>,
}

#[derive(custom_debug::Debug)]
pub(crate) struct Sink {
    pub(crate) id: Id,
    pub(crate) rx: router::ChannelReceiver,
    #[debug(skip)]
    pub(crate) stream: Box<dyn AsyncWrite + Send + Unpin>,
    pub(crate) pty_name: Option<String>,
}
