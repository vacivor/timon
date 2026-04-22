use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Deserializer, Serialize};

use crate::models::{Identity, Key as SshKey, KnownHostEntry, Profile, ProfileType};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub database: PathBuf,
    pub settings: PathBuf,
    pub known_hosts: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let home = user_home_dir().context("无法定位用户目录")?;
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

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            if drive.is_empty() || path.is_empty() {
                None
            } else {
                Some(PathBuf::from(drive).join(path))
            }
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub terminal: TerminalSettings,
    #[serde(default)]
    pub shortcuts: ShortcutSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            terminal: TerminalSettings::default(),
            shortcuts: ShortcutSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShortcutSettings {
    pub close_tab: String,
    pub tab_switches: [String; 9],
    pub previous_tab: String,
    pub next_tab: String,
    pub open_settings: String,
    pub minimize_window: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            close_tab: "Command+W".into(),
            tab_switches: [
                "Command+1".into(),
                "Command+2".into(),
                "Command+3".into(),
                "Command+4".into(),
                "Command+5".into(),
                "Command+6".into(),
                "Command+7".into(),
                "Command+8".into(),
                "Command+9".into(),
            ],
            previous_tab: "Command+Shift+[".into(),
            next_tab: "Command+Shift+]".into(),
            open_settings: "Command+,".into(),
            minimize_window: "Command+M".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSettings {
    pub default_theme_id: String,
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,
    pub font: FontSettings,
    pub cursor: CursorSettings,
    pub colors: TerminalColors,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            default_theme_id: "atom-one-light".into(),
            scrollback_lines: default_scrollback_lines(),
            font: FontSettings::default(),
            cursor: CursorSettings::default(),
            colors: TerminalColors::atom_one_light(),
        }
    }
}

fn default_scrollback_lines() -> usize {
    10_000
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
    pub cursor_color: String,
    pub cursor_text: String,
    pub selection_background: String,
    pub selection_foreground: String,
    pub normal: TerminalAnsiGroup,
    pub bright: TerminalAnsiGroup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalAnsiGroup {
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
}

impl TerminalAnsiGroup {
    pub fn as_array(&self) -> [String; 8] {
        [
            self.black.clone(),
            self.red.clone(),
            self.green.clone(),
            self.yellow.clone(),
            self.blue.clone(),
            self.magenta.clone(),
            self.cyan.clone(),
            self.white.clone(),
        ]
    }

    pub fn from_array(values: [String; 8]) -> Self {
        let [black, red, green, yellow, blue, magenta, cyan, white] = values;
        Self {
            black,
            red,
            green,
            yellow,
            blue,
            magenta,
            cyan,
            white,
        }
    }
}

impl TerminalColors {
    pub fn atom_one_light() -> Self {
        Self {
            background: "#fafafa".into(),
            foreground: "#383a42".into(),
            cursor_color: "#526fff".into(),
            cursor_text: "#fafafa".into(),
            selection_background: "#dbe9ff".into(),
            selection_foreground: "#1f2329".into(),
            normal: TerminalAnsiGroup {
                black: "#000000".into(),
                red: "#e45649".into(),
                green: "#50a14f".into(),
                yellow: "#c18401".into(),
                blue: "#4078f2".into(),
                magenta: "#a626a4".into(),
                cyan: "#0184bc".into(),
                white: "#a0a1a7".into(),
            },
            bright: TerminalAnsiGroup {
                black: "#696c77".into(),
                red: "#df6c75".into(),
                green: "#6aaf69".into(),
                yellow: "#e4c07b".into(),
                blue: "#61afef".into(),
                magenta: "#c678dd".into(),
                cyan: "#56b6c2".into(),
                white: "#ffffff".into(),
            },
        }
    }
}

pub struct Database {
    path: PathBuf,
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut statement = connection.prepare(&pragma)?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let exists = columns
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .any(|name| name == column);

    if !exists {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }

    Ok(())
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
                key_id INTEGER,
                identity_id INTEGER,
                host TEXT NOT NULL DEFAULT '',
                port INTEGER NOT NULL DEFAULT 22,
                username TEXT NOT NULL DEFAULT '',
                password TEXT NOT NULL DEFAULT '',
                theme_id TEXT NOT NULL DEFAULT 'default',
                shell_path TEXT NOT NULL DEFAULT '',
                work_dir TEXT NOT NULL DEFAULT '',
                startup_command TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT 'ssh'
            );

            CREATE TABLE IF NOT EXISTS keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                private_key TEXT NOT NULL DEFAULT '',
                public_key TEXT NOT NULL DEFAULT '',
                certificate TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                username TEXT NOT NULL DEFAULT '',
                password TEXT NOT NULL DEFAULT '',
                key_id INTEGER
            );
            "#,
        )?;
        ensure_column(&connection, "profiles", "key_id", "INTEGER")?;
        ensure_column(
            &connection,
            "profiles",
            "shell_path",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        ensure_column(
            &connection,
            "profiles",
            "work_dir",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        ensure_column(&connection, "identities", "key_id", "INTEGER")?;
        ensure_column(
            &connection,
            "keys",
            "certificate",
            "TEXT NOT NULL DEFAULT ''",
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
            "SELECT
                profiles.id,
                profiles.name,
                profiles.group_id,
                profiles.key_id,
                COALESCE(profiles.key_id, identities.key_id) AS effective_key_id,
                profiles.identity_id,
                profiles.host,
                profiles.port,
                profiles.username,
                CASE
                    WHEN trim(profiles.username) <> '' THEN profiles.username
                    ELSE COALESCE(identities.username, '')
                END AS display_username,
                profiles.password,
                profiles.theme_id,
                profiles.shell_path,
                profiles.work_dir,
                profiles.startup_command,
                profiles.type
             FROM profiles
             LEFT JOIN identities ON identities.id = profiles.identity_id
             ORDER BY profiles.id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Profile {
                id: row.get(0)?,
                name: row.get(1)?,
                group_id: row.get(2)?,
                key_id: row.get(3)?,
                effective_key_id: row.get(4)?,
                identity_id: row.get(5)?,
                host: row.get(6)?,
                port: row.get(7)?,
                username: row.get(8)?,
                display_username: row.get(9)?,
                password: row.get(10)?,
                theme_id: row.get(11)?,
                shell_path: row.get(12)?,
                work_dir: row.get(13)?,
                startup_command: row.get(14)?,
                profile_type: ProfileType::from(row.get_ref(15)?.as_str()?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_keys(&self) -> Result<Vec<SshKey>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, name, private_key, public_key, certificate FROM keys ORDER BY id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(SshKey {
                id: row.get(0)?,
                name: row.get(1)?,
                private_key: row.get(2)?,
                public_key: row.get(3)?,
                certificate: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_identities(&self) -> Result<Vec<Identity>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, name, username, password, key_id FROM identities ORDER BY id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Identity {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                password: row.get(3)?,
                key_id: row.get(4)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn save_profile(&self, profile: &mut Profile) -> Result<()> {
        let connection = self.open()?;

        if profile.id == 0 {
            connection.execute(
                "INSERT INTO profiles (name, group_id, key_id, identity_id, host, port, username, password, theme_id, shell_path, work_dir, startup_command, type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    profile.name,
                    profile.group_id,
                    profile.key_id,
                    profile.identity_id,
                    profile.host,
                    profile.port,
                    profile.username,
                    profile.password,
                    profile.theme_id,
                    profile.shell_path,
                    profile.work_dir,
                    profile.startup_command,
                    profile.profile_type.as_str(),
                ],
            )?;
            profile.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE profiles
                 SET name=?1, group_id=?2, key_id=?3, identity_id=?4, host=?5, port=?6, username=?7, password=?8, theme_id=?9, shell_path=?10, work_dir=?11, startup_command=?12, type=?13
                 WHERE id=?14",
                params![
                    profile.name,
                    profile.group_id,
                    profile.key_id,
                    profile.identity_id,
                    profile.host,
                    profile.port,
                    profile.username,
                    profile.password,
                    profile.theme_id,
                    profile.shell_path,
                    profile.work_dir,
                    profile.startup_command,
                    profile.profile_type.as_str(),
                    profile.id,
                ],
            )?;
        }

        Ok(())
    }

    pub fn save_key(&self, key: &mut SshKey) -> Result<()> {
        let connection = self.open()?;

        if key.id == 0 {
            connection.execute(
                "INSERT INTO keys (name, private_key, public_key, certificate) VALUES (?1, ?2, ?3, ?4)",
                params![
                    key.name,
                    key.private_key,
                    key.public_key,
                    key.certificate
                ],
            )?;
            key.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE keys SET name=?1, private_key=?2, public_key=?3, certificate=?4 WHERE id=?5",
                params![
                    key.name,
                    key.private_key,
                    key.public_key,
                    key.certificate,
                    key.id,
                ],
            )?;
        }

        Ok(())
    }

    pub fn save_identity(&self, identity: &mut Identity) -> Result<()> {
        let connection = self.open()?;

        if identity.id == 0 {
            connection.execute(
                "INSERT INTO identities (name, username, password, key_id) VALUES (?1, ?2, ?3, ?4)",
                params![
                    identity.name,
                    identity.username,
                    identity.password,
                    identity.key_id,
                ],
            )?;
            identity.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE identities SET name=?1, username=?2, password=?3, key_id=?4 WHERE id=?5",
                params![
                    identity.name,
                    identity.username,
                    identity.password,
                    identity.key_id,
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
