use self::protocol::Frame;
use futures_util::SinkExt;
use tokio::{prelude::*, stream::StreamExt};
use tokio_serde::{formats::SymmetricalBincode, SymmetricallyFramed};
use tokio_util::codec::{self, LengthDelimitedCodec};

pub mod local;
pub mod protocol;
pub mod remote;
pub mod terminal;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub type FramedWrite<T> = SymmetricallyFramed<
    codec::FramedWrite<T, LengthDelimitedCodec>,
    Frame,
    SymmetricalBincode<Frame>,
>;

pub type FramedRead<T> = SymmetricallyFramed<
    codec::FramedRead<T, LengthDelimitedCodec>,
    Frame,
    SymmetricalBincode<Frame>,
>;

impl Frame {
    pub fn new_writer<T>(inner: T) -> FramedWrite<T>
    where
        T: AsyncWrite,
    {
        let length_delimited = codec::FramedWrite::new(inner, LengthDelimitedCodec::new());
        SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
    }

    pub fn new_reader<T>(inner: T) -> FramedRead<T>
    where
        T: AsyncRead,
    {
        let length_delimited = codec::FramedRead::new(inner, LengthDelimitedCodec::new());
        SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
    }
}

pub async fn receiver(
    source: impl AsyncRead + Unpin,
    mut sink: impl AsyncWrite + Unpin,
) -> Result<()> {
    let mut reader = Frame::new_reader(source);
    while let Some(frame) = reader.next().await {
        match frame? {
            Frame::Output(output) => {
                sink.write_all(&output.data[..]).await?;
                sink.flush().await?;
            }
            frame => panic!("{:?}", frame),
        }
    }
    Ok(())
}

pub async fn sender(
    mut source: impl AsyncRead + Unpin,
    sink: impl AsyncWrite + Unpin,
) -> Result<()> {
    let mut writer = Frame::new_writer(sink);
    let mut buf = vec![0u8; 4096];
    loop {
        let n = source.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }

        let frame = Frame::Output(protocol::Output {
            id: protocol::Id(0),
            data: buf[..n].into(),
        });
        writer.send(frame).await.unwrap();
    }
    Ok(())
}

fn nix2io(e: nix::Error) -> io::Error {
    e.as_errno().unwrap().into()
}
