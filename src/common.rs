use crate::protocol::RemoteCommand;
use tokio::prelude::*;
use tokio_serde::{formats::SymmetricalBincode, SymmetricallyFramed};
use tokio_util::codec::{self, LengthDelimitedCodec};

pub(crate) type FramedWrite<T> = SymmetricallyFramed<
    codec::FramedWrite<T, LengthDelimitedCodec>,
    RemoteCommand,
    SymmetricalBincode<RemoteCommand>,
>;

pub(crate) type FramedRead<T> = SymmetricallyFramed<
    codec::FramedRead<T, LengthDelimitedCodec>,
    RemoteCommand,
    SymmetricalBincode<RemoteCommand>,
>;

impl RemoteCommand {
    pub(crate) fn new_writer<T>(inner: T) -> FramedWrite<T>
    where
        T: AsyncWrite,
    {
        let length_delimited = codec::FramedWrite::new(inner, LengthDelimitedCodec::new());
        SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
    }

    pub(crate) fn new_reader<T>(inner: T) -> FramedRead<T>
    where
        T: AsyncRead,
    {
        let length_delimited = codec::FramedRead::new(inner, LengthDelimitedCodec::new());
        SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
    }
}

pub(crate) fn nix2io(e: nix::Error) -> io::Error {
    e.as_errno().unwrap().into()
}
