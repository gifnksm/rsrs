use crate::{protocol, Result};
use etc_passwd::Passwd;
use futures_core::Stream;
use futures_util::{
    pin_mut,
    sink::{Sink, SinkExt},
    StreamExt,
};
use generational_arena::{Arena, Index};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::{hash_map::Entry, HashMap};
use std::{env, ffi::OsString, os::unix::process::CommandExt, process::Command as StdCommand};
use tokio::{prelude::*, process::Command, sync::mpsc};
use tokio_pty_command::{CommandExt as _, PtyMaster};
use tokio_serde::{formats::SymmetricalBincode, SymmetricallyFramed};
use tokio_util::codec::{self, LengthDelimitedCodec};

static ROUTER: Lazy<Mutex<Router>> = Lazy::new(|| Mutex::new(Router::new()));

#[derive(Debug)]
struct Router {
    tx: Option<mpsc::Sender<protocol::Frame>>,
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

    fn insert(&mut self, id: protocol::Id) -> Option<Receiver> {
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

    fn remove(&mut self, index: Index) -> Option<(protocol::Id, mpsc::Sender<protocol::Output>)> {
        self.channels.remove(index).map(|(id, tx)| {
            let _ = self.id_map.remove(&id).expect("corrupt internal status");
            (id, tx)
        })
    }

    fn get_mut(
        &mut self,
        id: protocol::Id,
    ) -> Option<(protocol::Id, mpsc::Sender<protocol::Output>)> {
        let channels = &mut self.channels;
        self.id_map
            .get(&id)
            .and_then(|index| channels.get_mut(*index))
            .cloned()
    }
}

// TODO: abstract as endpoint

#[derive(Debug)]
struct Receiver {
    index: Index,
    rx: mpsc::Receiver<protocol::Output>,
}

impl Drop for Receiver {
    fn drop(&mut self) {
        let mut router = ROUTER.lock();
        router.remove(self.index);
    }
}

pub async fn sender(remote_stream: impl AsyncWrite) -> Result<()> {
    let (tx, mut rx) = mpsc::channel(64);
    ROUTER.lock().tx = Some(tx);
    let frames = send_frames(remote_stream);
    pin_mut!(frames);

    while let Some(frame) = rx.next().await {
        frames.send(frame).await?;
    }

    Ok(())
}

pub async fn receiver(remote_stream: impl AsyncRead) -> Result<()> {
    let frames = received_frames(remote_stream);
    pin_mut!(frames);

    while let Some(frame) = frames.next().await {
        match frame? {
            protocol::Frame::Spawn(spawn) => {
                let rx = ROUTER
                    .lock()
                    .insert(spawn.id)
                    .expect("received id already used");
                tokio::spawn(async move {
                    // FIXME: error handling
                    let _ = spawn_process(rx, spawn).await;
                });
            }
            protocol::Frame::Output(output) => {
                let sender = ROUTER.lock().get_mut(output.id);
                if let Some((_, mut tx)) = sender {
                    tx.send(output).await?;
                }
            }
        }
    }
    Ok(())
}

fn received_frames(stream: impl AsyncRead) -> impl Stream<Item = io::Result<protocol::Frame>> {
    let length_delimited = codec::FramedRead::new(stream, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}

fn send_frames(stream: impl AsyncWrite) -> impl Sink<protocol::Frame, Error = io::Error> {
    let length_delimited = codec::FramedWrite::new(stream, LengthDelimitedCodec::new());
    SymmetricallyFramed::new(length_delimited, SymmetricalBincode::default())
}

async fn spawn_process(rx: Receiver, spawn: protocol::Spawn) -> Result<()> {
    let shell = if let Some(passwd) = Passwd::current_user()? {
        OsString::from(passwd.shell.to_str()?)
    } else if let Some(shell) = env::var_os("SHELL") {
        shell
    } else {
        panic!("cannot get login shell for the user");
    };

    let mut arg0 = OsString::from("-");
    arg0.push(&shell);

    let pty_master = PtyMaster::open()?;

    let mut std_command = StdCommand::new(shell);
    std_command.arg0(arg0);
    let child = Command::from(std_command).spawn_with_pty(&pty_master)?;

    let (child_stdout, child_stdin) = io::split(pty_master);
    let status = child;

    tokio::spawn(async move {
        let mut rx = rx;
        let mut child_stdin = child_stdin;
        while let Some(output) = rx.rx.next().await {
            // FIXME: error handling
            child_stdin.write_all(&output.data[..]).await.unwrap();
            child_stdin.flush().await.unwrap();
        }
    });

    let tx = ROUTER.lock().tx.clone().unwrap();
    tokio::spawn(async move {
        let mut tx = tx;
        let mut child_stdout = child_stdout;
        let mut buf = vec![0u8; 4096];
        loop {
            // FIXME: error handling
            let n = child_stdout.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }

            let frame = protocol::Frame::Output(protocol::Output {
                id: spawn.id,
                data: buf[..n].into(),
            });
            // FIXME: error handling
            tx.send(frame).await.unwrap();
        }
    });

    match status.await?.code() {
        Some(code) => eprintln!("remote process exited with {}", code),
        None => eprintln!("remote process terminated by signal"),
    }

    Ok(())
}
