use crate::{prelude::*, protocol, Result};
use generational_arena::{Arena, Index};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rand::prelude::*;
use std::{
    borrow::{Borrow, BorrowMut},
    collections::{hash_map::Entry, HashMap},
    fmt::Display,
    hash::Hash,
};

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
pub(crate) async fn setup(is_leaf: bool) -> Result<()> {
    if is_leaf {
        setup_leaf()
            .map_err(|e| e.wrap_err("failed to setup network"))
            .await?;
    } else {
        setup_root()
            .map_err(|e| e.wrap_err("failed to setup network"))
            .await?;
    }

    trace!("completed");
    Ok(())
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_leaf() -> Result<()> {
    // send magic number to rsrs-open
    let mut out = io::stdout();
    out.write_all(protocol::MAGIC).await?;
    out.flush().await?;

    // receive packet from rsrs-daemon

    Ok(())
}

#[tracing::instrument(err)]
#[allow(clippy::unit_arg)] // workaround for https://github.com/tokio-rs/tracing/issues/843
async fn setup_root() -> Result<()> {
    let mut store = NODE_STORE.lock();
    let (id, name) = store.insert()?;
    info!(%id, %name, "root daemon started");
    Ok(())
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct NodeId(Index);

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (idx, gen) = self.0.into_raw_parts();
        write!(f, "{}#{}", idx, gen)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct NodeName(String);

impl Display for NodeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Borrow<str> for NodeName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl BorrowMut<str> for NodeName {
    fn borrow_mut(&mut self) -> &mut str {
        &mut self.0
    }
}

impl From<String> for NodeName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug)]
struct Node {
    name: NodeName,
}

static NODE_STORE: Lazy<Mutex<NodeStore>> = Lazy::new(|| {
    Mutex::new(NodeStore {
        nodes: Arena::new(),
        by_name: HashMap::new(),
        name_gen: namegen::Generator::with_rng(StdRng::from_entropy()),
    })
});

#[derive(Debug)]
struct NodeStore {
    nodes: Arena<Node>,
    by_name: HashMap<NodeName, NodeId>,
    name_gen: namegen::Generator<'static, StdRng>,
}

impl NodeStore {
    fn insert_with_name(&mut self, name: impl Into<NodeName>) -> Result<NodeId> {
        let name = name.into();
        let e = match self.by_name.entry(name.clone()) {
            Entry::Occupied(_) => bail!("node name already used: {}", name),
            Entry::Vacant(e) => e,
        };
        let idx = self.nodes.insert(Node { name });
        let id = NodeId(idx);
        e.insert(id);
        Ok(id)
    }

    fn insert(&mut self) -> Result<(NodeId, NodeName)> {
        loop {
            let name = NodeName(self.name_gen.next().unwrap());
            match self.insert_with_name(name.clone()) {
                Ok(id) => return Ok((id, name)),
                Err(_) => continue,
            }
        }
    }

    fn get_id<Q>(&self, name: &Q) -> Option<NodeId>
    where
        NodeName: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.by_name.get(name).copied()
    }

    fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }
}
