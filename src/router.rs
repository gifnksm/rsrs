use crate::{endpoint, protocol, Error, Result};
use color_eyre::eyre::eyre;
use futures_core::{Future, Stream};
use futures_util::{
    future::TryFutureExt as _,
    pin_mut,
    sink::{Sink, SinkExt},
    StreamExt,
};
use generational_arena::{Arena, Index};
use once_cell::sync::Lazy;
use parking_lot::{Mutex, MutexGuard};
use std::{
    collections::{hash_map::Entry, HashMap},
    env,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

static ROUTER: Lazy<Mutex<Router>> = Lazy::new(|| Mutex::new(Router::new()));

#[derive(Debug)]
pub(crate) struct Router {
    kind: Option<protocol::ProcessKind>,
    id: usize,
    handler_tx: Option<mpsc::Sender<protocol::Command>>,
    channel_id_map: HashMap<protocol::Id, Index>,
    channels: Arena<(protocol::Id, mpsc::Sender<protocol::ChannelData>)>,
    status_id_map: HashMap<protocol::Id, Index>,
    status_notifiers: Arena<(protocol::Id, oneshot::Sender<protocol::ProcessExitStatus>)>,
}

impl Router {
    fn new() -> Self {
        Self {
            kind: None,
            id: 0,
            handler_tx: None,
            channel_id_map: HashMap::new(),
            channels: Arena::new(),
            status_id_map: HashMap::new(),
            status_notifiers: Arena::new(),
        }
    }

    pub(crate) fn insert_channel(&mut self, id: protocol::Id) -> Option<ChannelReceiver> {
        match self.channel_id_map.entry(id) {
            Entry::Vacant(e) => {
                // FIXME: implement back-pressure
                let (tx, rx) = mpsc::channel(64);
                let index = self.channels.insert((id, tx));
                e.insert(index);
                Some(ChannelReceiver { index, rx })
            }
            Entry::Occupied(_e) => None,
        }
    }

    fn remove_channel(
        &mut self,
        index: Index,
    ) -> Option<(protocol::Id, mpsc::Sender<protocol::ChannelData>)> {
        self.channels.remove(index).map(|(id, tx)| {
            let _ = self
                .channel_id_map
                .remove(&id)
                .expect("corrupt internal status");
            (id, tx)
        })
    }

    fn get_channel(
        &mut self,
        id: protocol::Id,
    ) -> Option<(protocol::Id, mpsc::Sender<protocol::ChannelData>)> {
        let channels = &mut self.channels;
        self.channel_id_map
            .get(&id)
            .and_then(|index| channels.get_mut(*index))
            .cloned()
    }

    pub(crate) fn insert_status_notifier(&mut self, id: protocol::Id) -> Option<StatusReceiver> {
        match self.status_id_map.entry(id) {
            Entry::Vacant(e) => {
                let (tx, rx) = oneshot::channel();
                let index = self.status_notifiers.insert((id, tx));
                e.insert(index);
                Some(StatusReceiver { index, rx })
            }
            Entry::Occupied(_e) => None,
        }
    }

    fn remove_status(
        &mut self,
        index: Index,
    ) -> Option<(protocol::Id, oneshot::Sender<protocol::ProcessExitStatus>)> {
        self.status_notifiers.remove(index).map(|(id, tx)| {
            let _ = self
                .status_id_map
                .remove(&id)
                .expect("corrupt internal status");
            (id, tx)
        })
    }

    fn take_status(
        &mut self,
        id: protocol::Id,
    ) -> Option<(protocol::Id, oneshot::Sender<protocol::ProcessExitStatus>)> {
        let notifiers = &mut self.status_notifiers;
        self.status_id_map
            .remove(&id)
            .and_then(|index| notifiers.remove(index))
    }

    pub(crate) fn handler_tx(&self) -> mpsc::Sender<protocol::Command> {
        self.handler_tx.clone().unwrap()
    }

    pub(crate) fn new_id(&mut self) -> protocol::Id {
        let id = self.id;
        self.id += 1;
        protocol::Id::new(self.kind.unwrap(), id)
    }
}

pub(crate) fn lock() -> MutexGuard<'static, Router> {
    ROUTER.lock()
}

#[derive(Debug)]
pub(crate) struct ChannelReceiver {
    index: Index,
    rx: mpsc::Receiver<protocol::ChannelData>,
}

impl Stream for ChannelReceiver {
    type Item = protocol::ChannelData;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

impl Drop for ChannelReceiver {
    fn drop(&mut self) {
        let mut router = ROUTER.lock();
        router.remove_channel(self.index);
    }
}

#[derive(Debug)]
pub(crate) struct StatusReceiver {
    index: Index,
    rx: oneshot::Receiver<protocol::ProcessExitStatus>,
}

impl Future for StatusReceiver {
    type Output = std::result::Result<protocol::ProcessExitStatus, oneshot::error::RecvError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.rx).poll(cx)
    }
}

impl Drop for StatusReceiver {
    fn drop(&mut self) {
        let mut router = ROUTER.lock();
        router.remove_status(self.index);
    }
}

async fn sender(
    sink: impl Sink<protocol::RemoteCommand, Error = Error>,
    mut peer_rx: mpsc::Receiver<protocol::RemoteCommand>,
) -> Result<()> {
    pin_mut!(sink);

    while let Some(frame) = peer_rx.next().await {
        sink.send(frame).await?;
    }

    Ok(())
}

async fn receiver(source: impl Stream<Item = Result<protocol::RemoteCommand>>) -> Result<()> {
    pin_mut!(source);

    while let Some(frame) = source.next().await {
        let frame = frame?;
        let mut tx = ROUTER.lock().handler_tx.clone().unwrap();
        tx.send(protocol::Command::Recv(frame))
            .map_err(|_| eyre!("send failed"))
            .await?;
    }
    Ok(())
}

async fn router(
    mut rx: mpsc::Receiver<protocol::Command>,
    mut peer_tx: mpsc::Sender<protocol::RemoteCommand>,
) -> Result<()> {
    while let Some(command) = rx.next().await {
        match command {
            protocol::Command::Recv(remote) => match remote {
                protocol::RemoteCommand::SetEnv(set_env) => {
                    for (k, v) in set_env.env_vars {
                        env::set_var(k, v);
                    }
                }
                protocol::RemoteCommand::Spawn(spawn) => {
                    let rx = ROUTER
                        .lock()
                        .insert_channel(spawn.id)
                        .expect("received id already used");
                    tokio::spawn(async move {
                        // FIXME: error handling
                        let _ = endpoint::process::run(rx, spawn).await;
                    });
                }
                protocol::RemoteCommand::Channel(protocol::ChannelCommand { id, data }) => {
                    let chan_tx = ROUTER.lock().get_channel(id);
                    if let Some((_, mut tx)) = chan_tx {
                        tx.send(data).await?;
                    }
                }
                protocol::RemoteCommand::ProcessExit(status) => {
                    let stat_tx = ROUTER.lock().take_status(status.id);
                    if let Some((_, tx)) = stat_tx {
                        // ignore error
                        let _ = tx.send(status);
                    }
                }
                protocol::RemoteCommand::Exit => break,
            },
            protocol::Command::Send(remote) => peer_tx.send(remote).await?,
            protocol::Command::Source(source) => {
                tokio::spawn(async move {
                    // FIXME: error handling
                    let _ = endpoint::source::run(source).await;
                });
            }
            protocol::Command::Sink(sink) => {
                tokio::spawn(async move {
                    // FIXME: error handling
                    let _ = endpoint::sink::run(sink).await;
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn spawn(
    kind: protocol::ProcessKind,
    source: impl Stream<Item = Result<protocol::RemoteCommand>> + Send + 'static,
    sink: impl Sink<protocol::RemoteCommand, Error = Error> + Send + 'static,
) -> JoinHandle<()> {
    let (handler_tx, handler_rx) = mpsc::channel(64);
    let (peer_tx, peer_rx) = mpsc::channel(64);
    ROUTER.lock().kind = Some(kind);
    ROUTER.lock().handler_tx = Some(handler_tx);

    tokio::spawn(async move {
        sender(sink, peer_rx).await.unwrap();
    });
    tokio::spawn(async move {
        receiver(source).await.unwrap();
    });
    tokio::spawn(async move {
        router(handler_rx, peer_tx).await.unwrap();
    })
}
