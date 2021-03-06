use std::ffi::OsString;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Request {
    Open(Open),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Open {
    pub(crate) pid: u32,
    pub(crate) command: OsString,
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Response {
    Ok,
    Err(String),
}
