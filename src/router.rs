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
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{prelude::*, sync::mpsc};
use tokio_serde::{formats::SymmetricalBincode, SymmetricallyFramed};
use tokio_util::codec::{self, LengthDelimitedCodec};

static ROUTER: Lazy<Mutex<Router>> = Lazy::new(|| Mutex::new(Router::new()));

#[derive(Debug)]
pub(crate) struct Router {
    tx: Option<mpsc::Sender<protocol::RemoteCommand>>,
    id_map: HashMap<protocol::Id, Index>,
    channels: Arena<(protocol::Id, mpsc::Sender<protocol::Output>)>,
}

impl Router {
    fn new() -> Self {
        Self {
            tx: None,
            id_map: HashMap::new(),
            channels: Arena::new(),
        }
    }

    pub(crate) fn insert(&mut self, id: protocol::Id) -> Option<Receiver> {
        match self.id_map.entry(id) {
            Entry::Vacant(e) => {
                // FIXME: implement backpressure
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
        self.tx.clone().unwrap()
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

pub async fn sender(stream: impl AsyncWrite) -> Result<()> {
    let (tx, mut rx) = mpsc::channel(64);
    ROUTER.lock().tx = Some(tx);
    let frames = send_frames(stream);
    pin_mut!(frames);

    while let Some(frame) = rx.next().await {
        frames.send(frame).await?;
    }

    Ok(())
}

pub async fn receiver(stream: impl AsyncRead) -> Result<()> {
    let frames = received_frames(stream);
    pin_mut!(frames);

    while let Some(frame) = frames.next().await {
        match frame? {
            protocol::RemoteCommand::Spawn(spawn) => {
                let rx = ROUTER
                    .lock()
                    .insert(spawn.id)
                    .expect("received id already used");
                tokio::spawn(async move {
                    // FIXME: error handling
                    let _ = endpoint::spawn::spawn_process(rx, spawn).await;
                });
            }
            protocol::RemoteCommand::Output(output) => {
                let sender = ROUTER.lock().get_mut(output.id);
                if let Some((_, mut tx)) = sender {
                    tx.send(output).await?;
                }
            }
        }
    }
    Ok(())
}

fn received_frames(
    stream: impl AsyncRead,
) -> impl Stream<Item = io::Result<protocol::RemoteCommand>> {
    let length_delimited = codec::FramedRead::new(stream, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}

fn send_frames(stream: impl AsyncWrite) -> impl Sink<protocol::RemoteCommand, Error = io::Error> {
    let length_delimited = codec::FramedWrite::new(stream, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}
