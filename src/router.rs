use crate::{endpoint, protocol, Result};
use futures_core::Stream;
use futures_util::{
    pin_mut,
    sink::{Sink, SinkExt},
    StreamExt,
};
use generational_arena::{Arena, Index};
use once_cell::sync::Lazy;
use parking_lot::{Mutex, MutexGuard};
use std::{
    collections::{hash_map::Entry, HashMap},
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{prelude::*, sync::mpsc};
use tokio_serde::{formats::SymmetricalBincode, SymmetricallyFramed};
use tokio_util::codec::{self, LengthDelimitedCodec};

static ROUTER: Lazy<Mutex<Router>> = Lazy::new(|| Mutex::new(Router::new()));

#[derive(Debug)]
pub(crate) struct Router {
    peer_tx: Option<mpsc::Sender<protocol::RemoteCommand>>,
    handler_tx: Option<mpsc::Sender<protocol::Command>>,
    id_map: HashMap<protocol::Id, Index>,
    channels: Arena<(protocol::Id, mpsc::Sender<protocol::Output>)>,
}

impl Router {
    fn new() -> Self {
        Self {
            peer_tx: None,
            handler_tx: None,
            id_map: HashMap::new(),
            channels: Arena::new(),
        }
    }

    pub(crate) fn insert(&mut self, id: protocol::Id) -> Option<Receiver> {
        match self.id_map.entry(id) {
            Entry::Vacant(e) => {
                // FIXME: implement back-pressure
                let (tx, rx) = mpsc::channel(64);
                let index = self.channels.insert((id, tx));
                e.insert(index);
                Some(Receiver { index, rx })
            }
            Entry::Occupied(_e) => None,
        }
    }

    pub(crate) fn remove(
        &mut self,
        index: Index,
    ) -> Option<(protocol::Id, mpsc::Sender<protocol::Output>)> {
        self.channels.remove(index).map(|(id, tx)| {
            let _ = self.id_map.remove(&id).expect("corrupt internal status");
            (id, tx)
        })
    }

    pub(crate) fn get_mut(
        &mut self,
        id: protocol::Id,
    ) -> Option<(protocol::Id, mpsc::Sender<protocol::Output>)> {
        let channels = &mut self.channels;
        self.id_map
            .get(&id)
            .and_then(|index| channels.get_mut(*index))
            .cloned()
    }

    pub(crate) fn sender(&self) -> mpsc::Sender<protocol::RemoteCommand> {
        self.peer_tx.clone().unwrap()
    }
}

pub(crate) fn lock() -> MutexGuard<'static, Router> {
    ROUTER.lock()
}

#[derive(Debug)]
pub(crate) struct Receiver {
    index: Index,
    rx: mpsc::Receiver<protocol::Output>,
}

impl Stream for Receiver {
    type Item = protocol::Output;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        let mut router = ROUTER.lock();
        router.remove(self.index);
    }
}

async fn sender(
    sink: impl Sink<protocol::RemoteCommand, Error = io::Error>,
    mut peer_rx: mpsc::Receiver<protocol::RemoteCommand>,
) -> Result<()> {
    pin_mut!(sink);

    while let Some(frame) = peer_rx.next().await {
        sink.send(frame).await?;
    }

    Ok(())
}

async fn receiver(source: impl Stream<Item = io::Result<protocol::RemoteCommand>>) -> Result<()> {
    pin_mut!(source);

    while let Some(frame) = source.next().await {
        let frame = frame?;
        let mut tx = ROUTER.lock().handler_tx.clone().unwrap();
        tx.send(protocol::Command::Remote(frame)).await?;
    }
    Ok(())
}

pub async fn handler(
    source: impl Stream<Item = io::Result<protocol::RemoteCommand>> + Send + 'static,
    sink: impl Sink<protocol::RemoteCommand, Error = io::Error> + Send + 'static,
) -> Result<()> {
    let (handler_tx, mut handler_rx) = mpsc::channel(64);
    let (peer_tx, peer_rx) = mpsc::channel(64);
    ROUTER.lock().peer_tx = Some(peer_tx);
    ROUTER.lock().handler_tx = Some(handler_tx);

    tokio::spawn(async move {
        sender(sink, peer_rx).await.unwrap();
    });
    tokio::spawn(async move {
        receiver(source).await.unwrap();
    });

    while let Some(command) = handler_rx.next().await {
        match command {
            protocol::Command::Local(local) => match local {
                protocol::LocalCommand::Source(source) => {
                    tokio::spawn(async move {
                        // FIXME: error handling
                        let _ = endpoint::source::run(source).await;
                    });
                }
                protocol::LocalCommand::Sink(sink) => {
                    tokio::spawn(async move {
                        // FIXME: error handling
                        let _ = endpoint::sink::run(sink).await;
                    });
                }
            },
            protocol::Command::Remote(remote) => match remote {
                protocol::RemoteCommand::Spawn(spawn) => {
                    let rx = ROUTER
                        .lock()
                        .insert(spawn.id)
                        .expect("received id already used");
                    tokio::spawn(async move {
                        // FIXME: error handling
                        let _ = endpoint::process::run(rx, spawn).await;
                    });
                }
                protocol::RemoteCommand::Output(output) => {
                    let sender = ROUTER.lock().get_mut(output.id);
                    if let Some((_, mut tx)) = sender {
                        tx.send(output).await?;
                    }
                }
            },
        }
    }
    Ok(())
}

pub fn received_frames(
    stream: impl AsyncRead,
) -> impl Stream<Item = io::Result<protocol::RemoteCommand>> {
    let length_delimited = codec::FramedRead::new(stream, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}

pub fn send_frames(
    stream: impl AsyncWrite,
) -> impl Sink<protocol::RemoteCommand, Error = io::Error> {
    let length_delimited = codec::FramedWrite::new(stream, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}
