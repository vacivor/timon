use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use rusqlite::{Connection as SqliteConnection, params};
use serde::{Deserialize, Deserializer, Serialize};

use crate::models::{
    Connection, ConnectionType, Group, Identity, Key as SshKey, KnownHostEntry, PortForward,
    PortForwardType,
};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub database: PathBuf,
    pub settings: PathBuf,
    pub known_hosts: PathBuf,
    pub themes: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let home = user_home_dir().context("无法定位用户目录")?;
        let root = home.join(".timon");
        let database = root.join("timon.sqlite3");
        let settings = root.join("settings.json");
        let known_hosts = root.join("known_hosts");
        let themes = root.join("themes");

        fs::create_dir_all(&root).with_context(|| format!("无法创建目录 {}", root.display()))?;
        fs::create_dir_all(&themes)
            .with_context(|| format!("无法创建目录 {}", themes.display()))?;

        Ok(Self {
            database,
            settings,
            known_hosts,
            themes,
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
    pub primary: TerminalPrimaryColors,
    pub cursor: TerminalCursorColors,
    pub selection: TerminalSelectionColors,
    pub normal: TerminalAnsiGroup,
    pub bright: TerminalAnsiGroup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalPrimaryColors {
    pub background: String,
    pub foreground: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalCursorColors {
    pub cursor: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSelectionColors {
    pub background: String,
    pub text: String,
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
            primary: TerminalPrimaryColors {
                background: "#fafafa".into(),
                foreground: "#383a42".into(),
            },
            cursor: TerminalCursorColors {
                cursor: "#526fff".into(),
                text: "#fafafa".into(),
            },
            selection: TerminalSelectionColors {
                background: "#dbe9ff".into(),
                text: "#1f2329".into(),
            },
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

    pub fn atom_one_dark() -> Self {
        Self {
            primary: TerminalPrimaryColors {
                background: "#1e2127".into(),
                foreground: "#abb2bf".into(),
            },
            cursor: TerminalCursorColors {
                cursor: "#61afef".into(),
                text: "#1e2127".into(),
            },
            selection: TerminalSelectionColors {
                background: "#3e4451".into(),
                text: "#d7dae0".into(),
            },
            normal: TerminalAnsiGroup {
                black: "#1e2127".into(),
                red: "#e06c75".into(),
                green: "#98c379".into(),
                yellow: "#e5c07b".into(),
                blue: "#61afef".into(),
                magenta: "#c678dd".into(),
                cyan: "#56b6c2".into(),
                white: "#abb2bf".into(),
            },
            bright: TerminalAnsiGroup {
                black: "#5c6370".into(),
                red: "#e06c75".into(),
                green: "#98c379".into(),
                yellow: "#e5c07b".into(),
                blue: "#61afef".into(),
                magenta: "#c678dd".into(),
                cyan: "#56b6c2".into(),
                white: "#ffffff".into(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalThemeEntry {
    pub id: String,
    pub path: PathBuf,
    pub colors: TerminalColors,
}

#[derive(Debug, Deserialize)]
struct ThemeFile {
    colors: TerminalColors,
}

pub fn builtin_terminal_themes() -> &'static [TerminalThemeEntry] {
    static THEMES: OnceLock<Vec<TerminalThemeEntry>> = OnceLock::new();
    THEMES
        .get_or_init(|| {
            vec![
                TerminalThemeEntry {
                    id: "atom-one-light".into(),
                    path: PathBuf::from("assets/themes/atom-one-light.toml"),
                    colors: parse_builtin_theme(
                        include_str!("../assets/themes/atom-one-light.toml"),
                        TerminalColors::atom_one_light,
                    ),
                },
                TerminalThemeEntry {
                    id: "atom-one-dark".into(),
                    path: PathBuf::from("assets/themes/atom-one-dark.toml"),
                    colors: parse_builtin_theme(
                        include_str!("../assets/themes/atom-one-dark.toml"),
                        TerminalColors::atom_one_dark,
                    ),
                },
            ]
        })
        .as_slice()
}

pub fn builtin_terminal_theme_by_id(theme_id: &str) -> Option<&'static TerminalThemeEntry> {
    builtin_terminal_themes()
        .iter()
        .find(|theme| theme.id == theme_id)
}

pub fn load_custom_terminal_themes(dir: &Path) -> Vec<TerminalThemeEntry> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut themes = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
        .filter_map(|path| {
            let id = path.file_stem()?.to_str()?.to_string();
            let source = fs::read_to_string(&path).ok()?;
            let file = toml::from_str::<ThemeFile>(&source).ok()?;
            Some(TerminalThemeEntry {
                id,
                path,
                colors: file.colors,
            })
        })
        .collect::<Vec<_>>();

    themes.sort_by(|left, right| left.id.cmp(&right.id));
    themes
}

fn parse_builtin_theme(source: &str, fallback: impl FnOnce() -> TerminalColors) -> TerminalColors {
    toml::from_str::<ThemeFile>(source)
        .map(|file| file.colors)
        .unwrap_or_else(|_| fallback())
}

pub struct Database {
    path: PathBuf,
}

fn ensure_column(
    connection: &SqliteConnection,
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

    fn open(&self) -> Result<SqliteConnection> {
        SqliteConnection::open(&self.path)
            .with_context(|| format!("无法打开数据库 {}", self.path.display()))
    }

    fn migrate(&self) -> Result<()> {
        let connection = self.open()?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS connections (
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
                serial_port TEXT NOT NULL DEFAULT '',
                baud_rate INTEGER NOT NULL DEFAULT 115200,
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

            CREATE TABLE IF NOT EXISTS groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                parent_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS port_forwards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                label TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT 'local',
                enabled INTEGER NOT NULL DEFAULT 0,
                bind_address TEXT NOT NULL DEFAULT '127.0.0.1',
                bind_port INTEGER NOT NULL DEFAULT 0,
                connection_id INTEGER,
                destination_host TEXT NOT NULL DEFAULT '',
                destination_port INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )?;
        ensure_column(&connection, "connections", "key_id", "INTEGER")?;
        ensure_column(
            &connection,
            "connections",
            "shell_path",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        ensure_column(
            &connection,
            "connections",
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
        ensure_column(&connection, "groups", "parent_id", "INTEGER")?;
        ensure_column(&connection, "port_forwards", "connection_id", "INTEGER")?;
        ensure_column(
            &connection,
            "connections",
            "serial_port",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        ensure_column(
            &connection,
            "connections",
            "baud_rate",
            "INTEGER NOT NULL DEFAULT 115200",
        )?;
        Ok(())
    }

    fn seed_defaults(&self) -> Result<()> {
        if self.list_connections()?.is_empty() {
            let mut ssh = Connection {
                name: "Example SSH".into(),
                host: "127.0.0.1".into(),
                username: "root".into(),
                ..Connection::default()
            };
            self.save_connection(&mut ssh)?;

            let mut local = Connection {
                name: "Local Shell".into(),
                host: "localhost".into(),
                theme_id: "atom-one-light".into(),
                connection_type: ConnectionType::Local,
                port: 0,
                ..Connection::default()
            };
            self.save_connection(&mut local)?;
        }

        Ok(())
    }

    pub fn list_connections(&self) -> Result<Vec<Connection>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT
                connections.id,
                connections.name,
                connections.group_id,
                connections.key_id,
                COALESCE(connections.key_id, identities.key_id) AS effective_key_id,
                connections.identity_id,
                connections.host,
                connections.port,
                connections.username,
                CASE
                    WHEN trim(connections.username) <> '' THEN connections.username
                    ELSE COALESCE(identities.username, '')
                END AS display_username,
                connections.password,
                connections.theme_id,
                connections.shell_path,
                connections.work_dir,
                connections.startup_command,
                connections.serial_port,
                connections.baud_rate,
                connections.type
             FROM connections
             LEFT JOIN identities ON identities.id = connections.identity_id
             ORDER BY connections.id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Connection {
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
                serial_port: row.get(15)?,
                baud_rate: row.get(16)?,
                connection_type: ConnectionType::from(row.get_ref(17)?.as_str()?),
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

    pub fn list_groups(&self) -> Result<Vec<Group>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, name, parent_id
             FROM groups
             ORDER BY COALESCE(parent_id, id), parent_id IS NOT NULL, name COLLATE NOCASE ASC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_port_forwards(&self) -> Result<Vec<PortForward>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT
                pf.id,
                pf.label,
                pf.type,
                pf.enabled,
                pf.bind_address,
                pf.bind_port,
                pf.connection_id,
                COALESCE(connections.name, '') AS connection_name,
                pf.destination_host,
                pf.destination_port
             FROM port_forwards pf
             LEFT JOIN connections ON connections.id = pf.connection_id
             ORDER BY pf.id DESC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(PortForward {
                id: row.get(0)?,
                label: row.get(1)?,
                forward_type: PortForwardType::from(row.get_ref(2)?.as_str()?),
                enabled: row.get::<_, i64>(3)? != 0,
                bind_address: row.get(4)?,
                bind_port: row.get(5)?,
                connection_id: row.get(6)?,
                connection_name: row.get(7)?,
                destination_host: row.get(8)?,
                destination_port: row.get(9)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn save_connection(&self, connection_model: &mut Connection) -> Result<()> {
        let connection = self.open()?;

        if connection_model.id == 0 {
            connection.execute(
                "INSERT INTO connections (name, group_id, key_id, identity_id, host, port, username, password, theme_id, shell_path, work_dir, startup_command, serial_port, baud_rate, type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    connection_model.name,
                    connection_model.group_id,
                    connection_model.key_id,
                    connection_model.identity_id,
                    connection_model.host,
                    connection_model.port,
                    connection_model.username,
                    connection_model.password,
                    connection_model.theme_id,
                    connection_model.shell_path,
                    connection_model.work_dir,
                    connection_model.startup_command,
                    connection_model.serial_port,
                    connection_model.baud_rate,
                    connection_model.connection_type.as_str(),
                ],
            )?;
            connection_model.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE connections
                 SET name=?1, group_id=?2, key_id=?3, identity_id=?4, host=?5, port=?6, username=?7, password=?8, theme_id=?9, shell_path=?10, work_dir=?11, startup_command=?12, serial_port=?13, baud_rate=?14, type=?15
                 WHERE id=?16",
                params![
                    connection_model.name,
                    connection_model.group_id,
                    connection_model.key_id,
                    connection_model.identity_id,
                    connection_model.host,
                    connection_model.port,
                    connection_model.username,
                    connection_model.password,
                    connection_model.theme_id,
                    connection_model.shell_path,
                    connection_model.work_dir,
                    connection_model.startup_command,
                    connection_model.serial_port,
                    connection_model.baud_rate,
                    connection_model.connection_type.as_str(),
                    connection_model.id,
                ],
            )?;
        }

        Ok(())
    }

    pub fn delete_connection(&self, connection_id: i64) -> Result<()> {
        let connection = self.open()?;

        connection.execute(
            "DELETE FROM port_forwards WHERE connection_id = ?1",
            params![connection_id],
        )?;
        connection.execute(
            "DELETE FROM connections WHERE id = ?1",
            params![connection_id],
        )?;

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

    pub fn save_group(&self, group: &mut Group) -> Result<()> {
        let connection = self.open()?;

        if group.id == 0 {
            connection.execute(
                "INSERT INTO groups (name, parent_id) VALUES (?1, ?2)",
                params![group.name.trim(), group.parent_id],
            )?;
            group.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE groups SET name=?1, parent_id=?2 WHERE id=?3",
                params![group.name.trim(), group.parent_id, group.id],
            )?;
        }

        Ok(())
    }

    pub fn save_port_forward(&self, forward: &mut PortForward) -> Result<()> {
        let connection = self.open()?;

        if forward.id == 0 {
            connection.execute(
                "INSERT INTO port_forwards (label, type, enabled, bind_address, bind_port, connection_id, destination_host, destination_port)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    forward.label,
                    forward.forward_type.as_str(),
                    i64::from(forward.enabled as i32),
                    forward.bind_address,
                    forward.bind_port,
                    forward.connection_id,
                    forward.destination_host,
                    forward.destination_port,
                ],
            )?;
            forward.id = connection.last_insert_rowid();
        } else {
            connection.execute(
                "UPDATE port_forwards
                 SET label=?1, type=?2, enabled=?3, bind_address=?4, bind_port=?5, connection_id=?6, destination_host=?7, destination_port=?8
                 WHERE id=?9",
                params![
                    forward.label,
                    forward.forward_type.as_str(),
                    i64::from(forward.enabled as i32),
                    forward.bind_address,
                    forward.bind_port,
                    forward.connection_id,
                    forward.destination_host,
                    forward.destination_port,
                    forward.id,
                ],
            )?;
        }

        Ok(())
    }

    pub fn delete_port_forward(&self, id: i64) -> Result<()> {
        let connection = self.open()?;
        connection.execute("DELETE FROM port_forwards WHERE id=?1", params![id])?;
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
