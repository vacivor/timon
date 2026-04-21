use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Deserializer, Serialize};

use crate::models::{Certificate, Identity, KnownHostEntry, Profile, ProfileType};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub database: PathBuf,
    pub settings: PathBuf,
    pub known_hosts: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("无法定位 HOME 目录")?;
        let root = home.join(".timon");
        let database = root.join("timon.sqlite3");
        let settings = root.join("settings.json");
        let known_hosts = root.join("known_hosts");

        fs::create_dir_all(&root).with_context(|| format!("无法创建目录 {}", root.display()))?;

        Ok(Self {
            database,
            settings,
            known_hosts,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub terminal: TerminalSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            terminal: TerminalSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSettings {
    pub default_theme_id: String,
    pub font: FontSettings,
    pub cursor: CursorSettings,
    pub colors: TerminalColors,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            default_theme_id: "atom-one-light".into(),
            font: FontSettings::default(),
            cursor: CursorSettings::default(),
            colors: TerminalColors::atom_one_light(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontSettings {
    pub family: String,
    pub size: f32,
    pub line_height: f32,
    #[serde(
        default = "default_font_thicken",
        deserialize_with = "deserialize_font_thicken"
    )]
    pub thicken: bool,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            family: "Menlo".into(),
            size: 14.0,
            line_height: 1.25,
            thicken: default_font_thicken(),
        }
    }
}

fn default_font_thicken() -> bool {
    false
}

fn deserialize_font_thicken<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FontThickenValue {
        Bool(bool),
        Number(f32),
    }

    Ok(match FontThickenValue::deserialize(deserializer)? {
        FontThickenValue::Bool(value) => value,
        FontThickenValue::Number(value) => value > 0.0,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorSettings {
    pub shape: String,
    pub blinking: bool,
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self {
            shape: "block".into(),
            blinking: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalColors {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub ansi_colors: [String; 16],
}

impl TerminalColors {
    pub fn atom_one_light() -> Self {
        Self {
            background: "#fafafa".into(),
            foreground: "#383a42".into(),
            cursor: "#526fff".into(),
            ansi_colors: [
                "#000000".into(),
                "#e45649".into(),
                "#50a14f".into(),
                "#c18401".into(),
                "#4078f2".into(),
                "#a626a4".into(),
                "#0184bc".into(),
                "#a0a1a7".into(),
                "#696c77".into(),
                "#df6c75".into(),
                "#6aaf69".into(),
                "#e4c07b".into(),
                "#61afef".into(),
                "#c678dd".into(),
                "#56b6c2".into(),
                "#ffffff".into(),
            ],
        }
    }
}

pub struct Database {
    path: PathBuf,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let database = Self {
            path: path.as_ref().to_path_buf(),
        };
        database.migrate()?;
        database.seed_defaults()?;
        Ok(database)
    }

    fn open(&self) -> Result<Connection> {
        Connection::open(&self.path)
            .with_context(|| format!("无法打开数据库 {}", self.path.display()))
    }

    fn migrate(&self) -> Result<()> {
        let connection = self.open()?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                group_id INTEGER,
                certificate_id INTEGER,
                identity_id INTEGER,
                host TEXT NOT NULL DEFAULT '',
                port INTEGER NOT NULL DEFAULT 22,
                username TEXT NOT NULL DEFAULT '',
                password TEXT NOT NULL DEFAULT '',
                theme_id TEXT NOT NULL DEFAULT 'default',
                startup_command TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT 'ssh'
            );

            CREATE TABLE IF NOT EXISTS certificates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                private_key TEXT NOT NULL DEFAULT '',
                public_key TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                username TEXT NOT NULL DEFAULT '',
                password TEXT NOT NULL DEFAULT '',
                certificate_id INTEGER
            );
            "#,
        )?;
        Ok(())
    }

    fn seed_defaults(&self) -> Result<()> {
        if self.list_profiles()?.is_empty() {
            let mut ssh = Profile {
                name: "Example SSH".into(),
                host: "127.0.0.1".into(),
                username: "root".into(),
                ..Profile::default()
            };
            self.save_profile(&mut ssh)?;

            let mut local = Profile {
                name: "Local Shell".into(),
                host: "localhost".into(),
                theme_id: "atom-one-light".into(),
                profile_type: ProfileType::Local,
                port: 0,
                ..Profile::default()
            };
            self.save_profile(&mut local)?;
        }

        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, name, group_id, certificate_id, identity_id, host, port, username, password, theme_id, startup_command, type
             FROM profiles
             ORDER BY id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Profile {
                id: row.get(0)?,
                name: row.get(1)?,
                group_id: row.get(2)?,
                certificate_id: row.get(3)?,
                identity_id: row.get(4)?,
                host: row.get(5)?,
                port: row.get(6)?,
                username: row.get(7)?,
                password: row.get(8)?,
                theme_id: row.get(9)?,
                startup_command: row.get(10)?,
                profile_type: ProfileType::from(row.get_ref(11)?.as_str()?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_certificates(&self) -> Result<Vec<Certificate>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, name, private_key, public_key FROM certificates ORDER BY id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Certificate {
                id: row.get(0)?,
                name: row.get(1)?,
                private_key: row.get(2)?,
                public_key: row.get(3)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_identities(&self) -> Result<Vec<Identity>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, name, username, password, certificate_id FROM identities ORDER BY id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Identity {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                password: row.get(3)?,
                certificate_id: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn save_profile(&self, profile: &mut Profile) -> Result<()> {
        let connection = self.open()?;

        if profile.id == 0 {
            connection.execute(
                "INSERT INTO profiles (name, group_id, certificate_id, identity_id, host, port, username, password, theme_id, startup_command, type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    profile.name,
                    profile.group_id,
                    profile.certificate_id,
                    profile.identity_id,
                    profile.host,
                    profile.port,
                    profile.username,
                    profile.password,
                    profile.theme_id,
                    profile.startup_command,
                    profile.profile_type.as_str(),
                ],
            )?;
            profile.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE profiles
                 SET name=?1, group_id=?2, certificate_id=?3, identity_id=?4, host=?5, port=?6, username=?7, password=?8, theme_id=?9, startup_command=?10, type=?11
                 WHERE id=?12",
                params![
                    profile.name,
                    profile.group_id,
                    profile.certificate_id,
                    profile.identity_id,
                    profile.host,
                    profile.port,
                    profile.username,
                    profile.password,
                    profile.theme_id,
                    profile.startup_command,
                    profile.profile_type.as_str(),
                    profile.id,
                ],
            )?;
        }

        Ok(())
    }

    pub fn save_certificate(&self, certificate: &mut Certificate) -> Result<()> {
        let connection = self.open()?;

        if certificate.id == 0 {
            connection.execute(
                "INSERT INTO certificates (name, private_key, public_key) VALUES (?1, ?2, ?3)",
                params![
                    certificate.name,
                    certificate.private_key,
                    certificate.public_key
                ],
            )?;
            certificate.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE certificates SET name=?1, private_key=?2, public_key=?3 WHERE id=?4",
                params![
                    certificate.name,
                    certificate.private_key,
                    certificate.public_key,
                    certificate.id,
                ],
            )?;
        }

        Ok(())
    }

    pub fn save_identity(&self, identity: &mut Identity) -> Result<()> {
        let connection = self.open()?;

        if identity.id == 0 {
            connection.execute(
                "INSERT INTO identities (name, username, password, certificate_id) VALUES (?1, ?2, ?3, ?4)",
                params![
                    identity.name,
                    identity.username,
                    identity.password,
                    identity.certificate_id,
                ],
            )?;
            identity.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE identities SET name=?1, username=?2, password=?3, certificate_id=?4 WHERE id=?5",
                params![
                    identity.name,
                    identity.username,
                    identity.password,
                    identity.certificate_id,
                    identity.id,
                ],
            )?;
        }

        Ok(())
    }
}

pub fn load_settings(path: &Path) -> Result<AppSettings> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let settings =
        fs::read_to_string(path).with_context(|| format!("无法读取设置文件 {}", path.display()))?;
    serde_json::from_str(&settings).context("设置文件格式无效")
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(settings)?;
    fs::write(path, content).with_context(|| format!("无法写入设置文件 {}", path.display()))
}

pub fn read_known_hosts(path: &Path) -> Result<Vec<KnownHostEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("无法读取 known_hosts 文件 {}", path.display()))?;

    Ok(content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            (!trimmed.is_empty() && !trimmed.starts_with('#')).then(|| KnownHostEntry {
                line_number: index + 1,
                line: trimmed.to_string(),
            })
        })
        .collect())
}
