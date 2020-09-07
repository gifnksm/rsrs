use crate::{
    common,
    prelude::*,
    protocol,
    protocol::network::{Handshake, HandshakeRsp, Message, NodeName},
    Result,
};
use common::{FdReader, FdWriter};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rand::prelude::*;
use std::{
    borrow::Borrow,
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
};
use tokio::{fs::File, io::BufReader, sync::mpsc};

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn setup(is_leaf: bool) -> Result<()> {
    if is_leaf {
        setup_leaf().await?
    } else {
        setup_root().await?
    };

    trace!("completed");
    Ok(())
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_leaf() -> Result<()> {
    let mut reader = File::open("/dev/stdin").await?;
    let mut writer = File::create("/dev/stdout").await?;

    // send magic number to rsrs-open
    trace!("sending magic number");
    writer.write_all(protocol::MAGIC).await?;
    writer.flush().await?;

    // receive handshake packet from rsrs-daemon
    trace!("receiving handshake request");
    let Handshake {
        server_name,
        client_name,
    } = common::new_reader(&mut reader).next().await.unwrap()?;
    trace!(%server_name, %client_name, "handshake received");

    // send handshake response
    trace!("sending handshake response");
    common::new_writer(&mut writer).send(HandshakeRsp).await?;
    debug!(%server_name, %client_name, "handshake completed");

    let mut reader = common::new_reader::<Message, _>(reader);
    let mut writer = common::new_writer::<Message, _>(writer);

    // TODO: specify appropriate buffer size
    let (tx, rx) = mpsc::channel(100);
    {
        // scope for lock guard
        let mut store = NODE_STORE.lock();
        store.insert_with_name(client_name.clone(), Node::MyNode)?;
        store.insert_with_name(server_name, Node::Connected { sender: tx })?;
    }

    Ok(())
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_root() -> Result<()> {
    let mut store = NODE_STORE.lock();
    let name = store.insert(Node::MyNode)?;
    info!(%name, "root daemon started");
    Ok(())
}

#[tracing::instrument(skip(remote_stdin, remote_stdout, remote_stderr), err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn connect_to_leaf(
    mut remote_stdin: FdWriter,
    mut remote_stdout: FdReader,
    remote_stderr: FdReader,
) -> Result<()> {
    let (client_name, server_name) = {
        // scope for lock guard
        let mut store = NODE_STORE.lock();
        let client_name = store.insert(Node::Handshake)?;
        let server_name = store.my_name.clone();
        assert!(!server_name.is_empty());
        (client_name, server_name)
    }; // lock ends here

    // forward stderr
    tokio::spawn({
        let client_name = client_name.clone();
        async move {
            let mut lines = BufReader::new(remote_stderr).lines();
            while let Some(line) = lines.next_line().await.unwrap() {
                eprintln!("[{}] {}", client_name, line);
            }
        }
    });

    trace!("sending handshake message to daemon");
    common::new_writer(&mut remote_stdin)
        .send(Handshake {
            server_name: server_name.clone(),
            client_name: client_name.clone(),
        })
        .await?;

    trace!("receiving handshake response from daemon");
    let HandshakeRsp = common::new_reader(&mut remote_stdout)
        .next()
        .await
        .unwrap()?;

    debug!(%server_name, %client_name, "handshake completed");

    let mut reader = common::new_reader::<Message, _>(remote_stdout);
    let mut writer = common::new_writer::<Message, _>(remote_stdin);

    // TODO: specify appropriate buffer size
    let (tx, mut rx) = mpsc::channel(100);

    {
        // scope for lock guard
        let mut store = NODE_STORE.lock();
        match store.get_mut(&client_name).unwrap() {
            node @ Node::Handshake => {
                *node = Node::Connected { sender: tx };
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

#[derive(custom_debug::Debug)]
enum Node {
    MyNode,
    Handshake,
    Connected { sender: mpsc::Sender<Message> },
}

static NODE_STORE: Lazy<Mutex<NodeStore>> = Lazy::new(|| {
    Mutex::new(NodeStore {
        my_name: NodeName::default(),
        nodes: HashMap::new(),
        name_gen: namegen::Generator::with_rng(StdRng::from_entropy()),
    })
});

#[derive(Debug)]
struct NodeStore {
    my_name: NodeName,
    nodes: HashMap<NodeName, Node>,
    name_gen: namegen::Generator<'static, StdRng>,
}

impl NodeStore {
    fn insert_with_name(&mut self, name: impl Into<NodeName>, node: Node) -> Result<()> {
        let name = name.into();
        assert!(!name.is_empty());

        if let Node::MyNode = node {
            if !self.my_name.is_empty() {
                bail!("my node name is already registered");
            }
            self.my_name = name.clone();
        }

        match self.nodes.entry(name) {
            Entry::Occupied(e) => bail!("node name already used: {}", e.key()),
            Entry::Vacant(e) => e.insert(node),
        };
        Ok(())
    }

    fn insert(&mut self, node: Node) -> Result<NodeName> {
        let name = loop {
            let name = NodeName::from(self.name_gen.next().unwrap());
            if !self.nodes.contains_key(&name) {
                break name;
            }
        };
        self.insert_with_name(name.clone(), node)?;
        Ok(name)
    }

    fn get<Q>(&self, name: &Q) -> Option<&Node>
    where
        NodeName: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.nodes.get(name)
    }

    fn get_mut<Q>(&mut self, name: &Q) -> Option<&mut Node>
    where
        NodeName: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.nodes.get_mut(name)
    }
}
