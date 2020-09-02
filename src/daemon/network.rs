use crate::{
    common,
    prelude::*,
    protocol,
    protocol::network::{Handshake, NodeName},
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

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn setup(is_leaf: bool) -> Result<NodeName> {
    let my_name = if is_leaf {
        setup_leaf().await?
    } else {
        setup_root().await?
    };

    trace!("completed");
    Ok(my_name)
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_leaf() -> Result<NodeName> {
    // send magic number to rsrs-open
    let mut out = io::stdout();
    out.write_all(protocol::MAGIC).await?;
    out.flush().await?;

    // receive handshake packet from rsrs-daemon
    let mut reader = common::new_reader::<Handshake, _>(io::stdin());

    let hs = reader.next().await.unwrap()?;
    let Handshake {
        server_name,
        client_name,
    } = hs;
    debug!(%server_name, %client_name, "handshake received");

    Ok(client_name)
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_root() -> Result<NodeName> {
    let mut store = NODE_STORE.lock();
    let name = store.insert(Node::MyNode)?;
    info!(%name, "root daemon started");
    Ok(name)
}

#[tracing::instrument(skip(reader, writer), err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn connect(reader: FdReader, mut writer: FdWriter) -> Result<()> {
    let (client_name, server_name) = {
        // scope for lock guard
        let mut store = NODE_STORE.lock();
        let client_name = store.insert(Node::Handshake)?;
        let server_name = store.my_name.clone();
        assert!(!server_name.is_empty());
        (client_name, server_name)
    }; // lock ends here

    let mut writer = common::new_writer::<Handshake, _>(&mut writer);
    trace!("sending handshake message to daemon");
    writer
        .send(Handshake {
            server_name,
            client_name,
        })
        .await?;

    Ok(())
}

#[derive(custom_debug::Debug)]
enum Node {
    MyNode,
    Handshake,
    Connected { reader: FdReader, writer: FdWriter },
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
}
