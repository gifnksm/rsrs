use crate::router;
use std::{ffi::OsString, os::unix::process::ExitStatusExt as _};
use tokio::prelude::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProcessKind {
    Local,
    Remote,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Id(ProcessKind, usize);

impl Id {
    pub(crate) fn new(kind: ProcessKind, id: usize) -> Self {
        Self(kind, id)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum RemoteCommand {
    SetEnv(SetEnv),
    Spawn(Spawn),
    Output(Output),
    ProcessExit(ProcessExitStatus),
    Exit,
}

#[derive(Debug)]
pub enum Command {
    Recv(RemoteCommand),
    Send(RemoteCommand),
    Source(Source),
    Sink(Sink),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SetEnv {
    pub env_vars: Vec<(OsString, OsString)>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum SpawnCommand {
    LoginShell,
    Program(OsString, Vec<OsString>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Spawn {
    pub id: Id,
    pub command: SpawnCommand,
    pub env_vars: Vec<(OsString, OsString)>,
    pub allocate_pty: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Output {
    pub id: Id,
    pub data: Vec<u8>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ProcessExitStatus {
    pub id: Id,
    pub status: ExitStatus,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ExitStatus {
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
pub struct Source {
    pub id: Id,
    #[debug(skip)]
    pub stream: Box<dyn AsyncRead + Send + Unpin>,
}

#[derive(custom_debug::Debug)]
pub struct Sink {
    pub id: Id,
    pub rx: router::ChannelReceiver,
    #[debug(skip)]
    pub stream: Box<dyn AsyncWrite + Send + Unpin>,
}
