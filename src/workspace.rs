use crate::models::{
    Connection, Group, Identity, Key as SshKey, KnownHostEntry, PortForward, Snippet,
};
use crate::persistence::{AppPaths, Database, read_known_hosts};

#[derive(Debug, Clone, Default)]
pub struct WorkspaceData {
    pub groups: Vec<Group>,
    pub connections: Vec<Connection>,
    pub keys: Vec<SshKey>,
    pub identities: Vec<Identity>,
    pub port_forwards: Vec<PortForward>,
    #[allow(dead_code)]
    pub snippets: Vec<Snippet>,
    pub known_hosts: Vec<KnownHostEntry>,
}

impl WorkspaceData {
    pub fn load(paths: &AppPaths, database: &Database) -> Self {
        Self {
            groups: database.list_groups().unwrap_or_default(),
            connections: database.list_connections().unwrap_or_default(),
            keys: database.list_keys().unwrap_or_default(),
            identities: database.list_identities().unwrap_or_default(),
            port_forwards: database.list_port_forwards().unwrap_or_default(),
            snippets: database.list_snippets().unwrap_or_default(),
            known_hosts: read_known_hosts(&paths.known_hosts).unwrap_or_default(),
        }
    }
}
