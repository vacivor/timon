use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    Ssh,
    Local,
    Serial,
}

impl ConnectionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Local => "local",
            Self::Serial => "serial",
        }
    }
}

impl Default for ConnectionType {
    fn default() -> Self {
        Self::Ssh
    }
}

impl From<&str> for ConnectionType {
    fn from(value: &str) -> Self {
        match value {
            "local" => Self::Local,
            "serial" => Self::Serial,
            _ => Self::Ssh,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: i64,
    pub name: String,
    pub group_id: Option<i64>,
    pub key_id: Option<i64>,
    pub effective_key_id: Option<i64>,
    pub identity_id: Option<i64>,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub display_username: String,
    pub password: String,
    pub theme_id: String,
    pub shell_path: String,
    pub work_dir: String,
    pub startup_command: String,
    pub serial_port: String,
    pub baud_rate: i64,
    pub connection_type: ConnectionType,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Connection".into(),
            group_id: None,
            key_id: None,
            effective_key_id: None,
            identity_id: None,
            host: String::new(),
            port: 22,
            username: String::new(),
            display_username: String::new(),
            password: String::new(),
            theme_id: "default".into(),
            shell_path: String::new(),
            work_dir: String::new(),
            startup_command: String::new(),
            serial_port: String::new(),
            baud_rate: 115200,
            connection_type: ConnectionType::Ssh,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Key {
    pub id: i64,
    pub name: String,
    pub private_key: String,
    pub public_key: String,
    pub certificate: String,
}

impl Default for Key {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Key".into(),
            private_key: String::new(),
            public_key: String::new(),
            certificate: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub password: String,
    pub key_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Group".into(),
            parent_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortForwardType {
    Local,
    Remote,
    Dynamic,
}

impl PortForwardType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::Dynamic => "dynamic",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Remote => "Remote",
            Self::Dynamic => "Dynamic",
        }
    }
}

impl Default for PortForwardType {
    fn default() -> Self {
        Self::Local
    }
}

impl From<&str> for PortForwardType {
    fn from(value: &str) -> Self {
        match value {
            "remote" => Self::Remote,
            "dynamic" => Self::Dynamic,
            _ => Self::Local,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForward {
    pub id: i64,
    pub label: String,
    pub forward_type: PortForwardType,
    pub enabled: bool,
    pub bind_address: String,
    pub bind_port: i64,
    pub connection_id: Option<i64>,
    pub connection_name: String,
    pub destination_host: String,
    pub destination_port: i64,
}

impl Default for PortForward {
    fn default() -> Self {
        Self {
            id: 0,
            label: "New Forward".into(),
            forward_type: PortForwardType::Local,
            enabled: false,
            bind_address: "127.0.0.1".into(),
            bind_port: 0,
            connection_id: None,
            connection_name: String::new(),
            destination_host: String::new(),
            destination_port: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SftpEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Identity".into(),
            username: String::new(),
            password: String::new(),
            key_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageMenu {
    Connections,
    Keychain,
    PortForwarding,
    Snippets,
    KnownHosts,
    Logs,
    Settings,
}

impl ManageMenu {
    pub const ALL: [ManageMenu; 7] = [
        ManageMenu::Connections,
        ManageMenu::Keychain,
        ManageMenu::PortForwarding,
        ManageMenu::Snippets,
        ManageMenu::KnownHosts,
        ManageMenu::Logs,
        ManageMenu::Settings,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Connections => "Connections",
            Self::Keychain => "Keychain",
            Self::PortForwarding => "Port Forwarding",
            Self::Snippets => "Snippets",
            Self::KnownHosts => "Known Hosts",
            Self::Logs => "Logs",
            Self::Settings => "Settings",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Connections => 0,
            Self::Keychain => 1,
            Self::PortForwarding => 2,
            Self::Snippets => 3,
            Self::KnownHosts => 4,
            Self::Logs => 5,
            Self::Settings => 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KnownHostEntry {
    pub line_number: usize,
    pub line: String,
}
