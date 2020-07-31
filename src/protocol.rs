use crate::router;
use tokio::prelude::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Id(pub usize);

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum RemoteCommand {
    Spawn(Spawn),
    Output(Output),
}

#[derive(Debug)]
pub(crate) enum LocalCommand {
    Source(Source),
    Sink(Sink),
}

#[derive(Debug)]
pub(crate) enum Command {
    Remote(RemoteCommand),
    Local(LocalCommand),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Spawn {
    pub id: Id,
    pub env_vars: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Output {
    pub id: Id,
    pub data: Vec<u8>,
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
    pub(crate) rx: router::Receiver,
    #[debug(skip)]
    pub(crate) stream: Box<dyn AsyncWrite + Send + Unpin>,
}
