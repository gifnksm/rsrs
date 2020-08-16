use std::ffi::OsString;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Request {
    Open(Open),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Open {
    pub(crate) command: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) has_stdin: bool,
    pub(crate) has_stdout: bool,
    pub(crate) has_stderr: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Response {
    Ok,
    Err,
}
