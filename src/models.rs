use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileType {
    Ssh,
    Local,
}

impl ProfileType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Local => "local",
        }
    }
}

impl Default for ProfileType {
    fn default() -> Self {
        Self::Ssh
    }
}

impl From<&str> for ProfileType {
    fn from(value: &str) -> Self {
        match value {
            "local" => Self::Local,
            _ => Self::Ssh,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub group_id: Option<i64>,
    pub certificate_id: Option<i64>,
    pub identity_id: Option<i64>,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub password: String,
    pub theme_id: String,
    pub startup_command: String,
    pub profile_type: ProfileType,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Profile".into(),
            group_id: None,
            certificate_id: None,
            identity_id: None,
            host: String::new(),
            port: 22,
            username: String::new(),
            password: String::new(),
            theme_id: "default".into(),
            startup_command: String::new(),
            profile_type: ProfileType::Ssh,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub id: i64,
    pub name: String,
    pub private_key: String,
    pub public_key: String,
}

impl Default for Certificate {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Certificate".into(),
            private_key: String::new(),
            public_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub password: String,
    pub certificate_id: Option<i64>,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            id: 0,
            name: "New Identity".into(),
            username: String::new(),
            password: String::new(),
            certificate_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageMenu {
    Profiles,
    Keychain,
    PortForwarding,
    Snippets,
    KnownHosts,
    Logs,
    Settings,
}

impl ManageMenu {
    pub const ALL: [ManageMenu; 7] = [
        ManageMenu::Profiles,
        ManageMenu::Keychain,
        ManageMenu::PortForwarding,
        ManageMenu::Snippets,
        ManageMenu::KnownHosts,
        ManageMenu::Logs,
        ManageMenu::Settings,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Profiles => "Profiles",
            Self::Keychain => "Keychain",
            Self::PortForwarding => "Port Forwarding",
            Self::Snippets => "Snippets",
            Self::KnownHosts => "Known Hosts",
            Self::Logs => "Logs",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone)]
pub struct KnownHostEntry {
    pub line_number: usize,
    pub line: String,
}
