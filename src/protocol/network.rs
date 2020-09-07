use std::{
    borrow::{Borrow, BorrowMut},
    fmt::Display,
    ops::Deref,
};

#[derive(
    Debug,
    Default,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub(crate) struct NodeName(String);

impl Display for NodeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Borrow<str> for NodeName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl BorrowMut<str> for NodeName {
    fn borrow_mut(&mut self) -> &mut str {
        &mut self.0
    }
}

impl From<String> for NodeName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl Deref for NodeName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Handshake {
    pub(crate) client_name: NodeName,
    pub(crate) server_name: NodeName,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct HandshakeRsp;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Message {
    src: NodeName,
    dst: NodeName,
}
