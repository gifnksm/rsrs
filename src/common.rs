use crate::prelude::*;
use serde::{Deserialize, Serialize};
use tokio_serde::{formats::SymmetricalBincode, SymmetricallyFramed};
use tokio_util::codec::{self, LengthDelimitedCodec};

pub(crate) type FramedWrite<T, S> =
    SymmetricallyFramed<codec::FramedWrite<S, LengthDelimitedCodec>, T, SymmetricalBincode<T>>;

pub(crate) type FramedRead<T, S> =
    SymmetricallyFramed<codec::FramedRead<S, LengthDelimitedCodec>, T, SymmetricalBincode<T>>;

pub(crate) fn new_writer<T, S>(inner: S) -> FramedWrite<T, S>
where
    T: Serialize,
    S: AsyncWrite,
{
    let length_delimited = codec::FramedWrite::new(inner, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}

pub(crate) fn new_reader<T, S>(inner: S) -> FramedRead<T, S>
where
    T: for<'a> Deserialize<'a>,
    S: AsyncRead,
{
    let length_delimited = codec::FramedRead::new(inner, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}

pub(crate) fn nix2io(e: nix::Error) -> io::Error {
    e.as_errno().unwrap().into()
}
