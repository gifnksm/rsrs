#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Id(pub usize);

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum RemoteCommand {
    Spawn(Spawn),
    Output(Output),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Spawn {
    pub id: Id,
    pub env_vars: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Output {
    pub id: Id,
    pub data: Vec<u8>,
}
