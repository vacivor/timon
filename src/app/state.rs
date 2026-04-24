use super::*;

pub(crate) struct App {
    pub(crate) paths: AppPaths,
    pub(crate) database: Database,
    pub(crate) settings: AppSettings,
    pub(crate) terminal_font: TerminalFont,
    pub(crate) glyph_atlas: Arc<Mutex<GlyphAtlas>>,
    pub(crate) main_window_scale_factor: f32,
    pub(crate) available_fonts: Vec<String>,
    pub(crate) available_shells: Vec<String>,
    pub(crate) terminal_themes: Vec<TerminalThemeEntry>,
    pub(crate) main_window: Option<window::Id>,
    pub(crate) settings_window: Option<window::Id>,
    pub(crate) main_window_size: iced::Size,
    pub(crate) cursor_position: Option<Point>,
    pub(crate) selected_menu: ManageMenu,
    pub(crate) sidebar_menu_progress: [f32; 7],
    pub(crate) active_tab: ActiveTab,
    pub(crate) manage_tab_width: f32,
    pub(crate) terminal_tabs: Vec<TerminalTab>,
    pub(crate) next_terminal_id: u64,
    pub(crate) drawer: Option<DrawerState>,
    pub(crate) context_menu: Option<ContextMenuState>,
    pub(crate) active_group_context: Option<i64>,
    pub(crate) active_port_forward_context: Option<i64>,
    pub(crate) terminal_focus: Option<u64>,
    pub(crate) terminal_composer_focus: Option<u64>,
    pub(crate) groups: Vec<Group>,
    pub(crate) connections: Vec<Connection>,
    pub(crate) keys: Vec<SshKey>,
    pub(crate) identities: Vec<Identity>,
    pub(crate) port_forwards: Vec<PortForward>,
    pub(crate) port_forward_runtimes: std::collections::BTreeMap<i64, PortForwardHandle>,
    pub(crate) known_hosts: Vec<KnownHostEntry>,
    pub(crate) logs: Vec<String>,
    pub(crate) settings_editor: SettingsEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextMenuTarget {
    Connection(i64),
    Key(i64),
    Identity(i64),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ContextMenuState {
    pub(crate) target: ContextMenuTarget,
    pub(crate) position: Option<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveTab {
    Manage,
    Terminal(u64),
}

pub(crate) struct TerminalTab {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) fallback_title: String,
    pub(crate) connection_type: ConnectionType,
    pub(crate) theme_id: String,
    pub(crate) status: String,
    pub(crate) titlebar_width: f32,
    pub(crate) workspace: TabWorkspace,
    pub(crate) terminal: TerminalView,
    pub(crate) composer: text_editor::Content,
    pub(crate) selection_anchor: Option<TerminalPoint>,
    pub(crate) selection: Option<TerminalSelection>,
    pub(crate) session: Option<SessionHandle>,
}

pub(crate) enum TabWorkspace {
    Terminal,
    Sftp(SftpTabState),
}

pub(crate) struct SftpTabState {
    pub(crate) handle: Option<SftpHandle>,
    pub(crate) current_path: String,
    pub(crate) entries: Vec<SftpEntry>,
    pub(crate) selected_file: Option<String>,
    pub(crate) preview: String,
    pub(crate) loading: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum DrawerState {
    Connection(ConnectionEditor),
    Group(GroupEditor),
    Key(KeyEditor),
    Identity(IdentityEditor),
    PortForward(PortForwardEditor),
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectionEditor {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) group_id: String,
    pub(crate) key_id: String,
    pub(crate) identity_id: String,
    pub(crate) host: String,
    pub(crate) port: String,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) theme_id: String,
    pub(crate) shell_path: String,
    pub(crate) work_dir: String,
    pub(crate) startup_command: String,
    pub(crate) serial_port: String,
    pub(crate) baud_rate: String,
    pub(crate) connection_type: ConnectionType,
}

#[derive(Debug, Clone)]
pub(crate) struct GroupEditor {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) parent_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct KeyEditor {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) private_key: String,
    pub(crate) private_key_content: text_editor::Content,
    pub(crate) public_key: String,
    pub(crate) public_key_content: text_editor::Content,
    pub(crate) certificate: String,
    pub(crate) certificate_content: text_editor::Content,
}

#[derive(Debug, Clone)]
pub(crate) struct IdentityEditor {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) key_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PortForwardEditor {
    pub(crate) id: Option<i64>,
    pub(crate) label: String,
    pub(crate) forward_type: PortForwardType,
    pub(crate) enabled: bool,
    pub(crate) bind_address: String,
    pub(crate) bind_port: String,
    pub(crate) connection_id: String,
    pub(crate) destination_host: String,
    pub(crate) destination_port: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SettingsEditor {
    pub(crate) font_family: String,
    pub(crate) font_size: String,
    pub(crate) line_height: String,
    pub(crate) scrollback_lines: String,
    pub(crate) font_thicken: bool,
    pub(crate) cursor_shape: String,
    pub(crate) cursor_blinking: bool,
    pub(crate) background: String,
    pub(crate) foreground: String,
    pub(crate) cursor_color: String,
    pub(crate) cursor_text: String,
    pub(crate) selection_background: String,
    pub(crate) selection_foreground: String,
    pub(crate) ansi_normal: [String; 8],
    pub(crate) ansi_bright: [String; 8],
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ConnectionField {
    Name,
    GroupId,
    KeyId,
    IdentityId,
    Host,
    Port,
    Username,
    Password,
    ThemeId,
    ShellPath,
    WorkDir,
    StartupCommand,
    SerialPort,
    BaudRate,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum KeyField {
    Name,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum KeyTextField {
    PrivateKey,
    PublicKey,
    Certificate,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum IdentityField {
    Name,
    Username,
    Password,
    KeyId,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum GroupField {
    Name,
    ParentId,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PortForwardField {
    Label,
    BindAddress,
    BindPort,
    ConnectionId,
    DestinationHost,
    DestinationPort,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FontField {
    Family,
    Size,
    LineHeight,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CursorField {
    Shape,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ColorField {
    Background,
    Foreground,
    CursorColor,
    CursorText,
    SelectionBackground,
    SelectionForeground,
    AnsiNormal(usize),
    AnsiBright(usize),
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    MainWindowReady(window::Id),
    SettingsWindowReady(window::Id),
    Tick,
    WindowEvent(window::Id, window::Event),
    CursorMoved(Point),
    KeyboardInput(window::Id, keyboard::Event),
    InputMethod(window::Id, input_method::Event),
    DragWindow(window::Id),
    SelectMenu(ManageMenu),
    OpenSettingsWindow,
    ActivateManageTab,
    ActivateTerminal(u64),
    CloseTerminal(u64),
    TerminalSelectionStarted(u64, TerminalPoint),
    TerminalSelectionUpdated(u64, TerminalPoint),
    TerminalScrolled(u64, i32, TerminalPoint),
    TerminalResized(u64, usize, usize),
    TerminalPaste(u64, Option<String>),
    TerminalComposerAction(u64, text_editor::Action),
    SubmitTerminalComposer(u64),
    OpenConnectionContext(i64),
    OpenKeyContext(i64),
    OpenIdentityContext(i64),
    DuplicateConnection(i64),
    DeleteConnection(i64),
    NewConnection,
    NewSerialConnection,
    EditConnection(i64),
    OpenSftpConnection(i64),
    SaveConnection,
    ConnectionFieldChanged(ConnectionField, String),
    ConnectionTypeChanged(String),
    NewGroup,
    EditGroup(i64),
    SaveGroup,
    GroupFieldChanged(GroupField, String),
    NewKey,
    EditKey(i64),
    SaveKey,
    KeyFieldChanged(KeyField, String),
    KeyEditorAction(KeyTextField, text_editor::Action),
    NewIdentity,
    EditIdentity(i64),
    SaveIdentity,
    IdentityFieldChanged(IdentityField, String),
    NewPortForward,
    EditPortForward(i64),
    SavePortForward,
    DeletePortForward(i64),
    TogglePortForward(i64, bool),
    PortForwardStarted(i64, Result<PortForwardHandle, String>),
    PortForwardFieldChanged(PortForwardField, String),
    PortForwardTypeChanged(String),
    SftpConnected(u64, Result<SftpHandle, String>),
    SftpDirectoryLoaded(u64, String, Result<Vec<SftpEntry>, String>),
    SftpFileLoaded(u64, String, Result<String, String>),
    SftpNavigate(u64, String),
    SftpOpenEntry(u64, String, bool),
    SftpOpenParent(u64),
    SftpRefresh(u64),
    CloseDrawer,
    ConnectConnection(i64),
    TerminalConnected(u64, Result<SessionHandle, String>),
    TerminalSessionEvent(u64, SessionEvent),
    SettingsFontChanged(FontField, String),
    SettingsScrollbackChanged(String),
    SettingsFontThickenChanged(bool),
    SettingsCursorChanged(CursorField, String),
    SettingsCursorBlinkChanged(bool),
    SettingsColorChanged(ColorField, String),
    SaveSettings,
    ResetThemeToAtomOneLight,
}

impl App {
    pub(crate) fn new(paths: AppPaths, database: Database, settings: AppSettings) -> Self {
        let available_fonts = available_terminal_fonts();
        let available_shells = available_local_shells();
        let mut terminal_themes = builtin_terminal_themes().to_vec();
        terminal_themes.extend(load_custom_terminal_themes(&paths.themes));
        terminal_themes.sort_by(|left, right| left.id.cmp(&right.id));
        let mut settings = settings;
        settings.terminal.font.family =
            normalize_font_family_choice(&settings.terminal.font.family, &available_fonts);
        let terminal_font = TerminalFont::from_settings(&settings.terminal.font);
        let glyph_atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
        let settings_editor = SettingsEditor::from_settings(&settings);
        let connections = database.list_connections().unwrap_or_default();
        let groups = database.list_groups().unwrap_or_default();
        let keys = database.list_keys().unwrap_or_default();
        let identities = database.list_identities().unwrap_or_default();
        let port_forwards = database.list_port_forwards().unwrap_or_default();
        let known_hosts = read_known_hosts(&paths.known_hosts).unwrap_or_default();

        let app = Self {
            paths,
            database,
            settings,
            terminal_font,
            glyph_atlas,
            main_window_scale_factor: 1.0,
            available_fonts,
            available_shells,
            terminal_themes,
            main_window: None,
            settings_window: None,
            main_window_size: iced::Size::new(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT),
            cursor_position: None,
            selected_menu: ManageMenu::Connections,
            sidebar_menu_progress: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            active_tab: ActiveTab::Manage,
            manage_tab_width: TITLEBAR_MANAGE_ACTIVE_WIDTH,
            terminal_tabs: Vec::new(),
            next_terminal_id: 1,
            drawer: None,
            context_menu: None,
            active_group_context: None,
            active_port_forward_context: None,
            terminal_focus: None,
            terminal_composer_focus: None,
            groups,
            connections,
            keys,
            identities,
            port_forwards,
            port_forward_runtimes: std::collections::BTreeMap::new(),
            known_hosts,
            logs: vec!["Timon 已启动".into()],
            settings_editor,
        };

        app.prewarm_terminal_glyphs();
        app
    }

    pub(crate) fn log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    pub(crate) fn reload_data(&mut self) {
        self.groups = self.database.list_groups().unwrap_or_default();
        self.connections = self.database.list_connections().unwrap_or_default();
        self.keys = self.database.list_keys().unwrap_or_default();
        self.identities = self.database.list_identities().unwrap_or_default();
        self.port_forwards = self.database.list_port_forwards().unwrap_or_default();
        self.known_hosts = read_known_hosts(&self.paths.known_hosts).unwrap_or_default();
    }

    pub(crate) fn terminal_font(&self) -> &TerminalFont {
        &self.terminal_font
    }

    pub(crate) fn terminal_theme(&self, theme_id: &str) -> TerminalTheme {
        let settings_theme = TerminalTheme::from_settings(&self.settings.terminal.colors);

        match theme_id {
            "default" => settings_theme,
            other if other == self.settings.terminal.default_theme_id => settings_theme,
            other => self
                .terminal_themes
                .iter()
                .find(|theme| theme.id == other)
                .or_else(|| builtin_terminal_theme_by_id(other))
                .map(|theme| TerminalTheme::from_settings(&theme.colors))
                .unwrap_or_else(|| {
                    self.terminal_themes
                        .iter()
                        .find(|theme| theme.id == "atom-one-light")
                        .or_else(|| builtin_terminal_theme_by_id("atom-one-light"))
                        .map(|theme| TerminalTheme::from_settings(&theme.colors))
                        .unwrap_or_else(|| {
                            TerminalTheme::from_settings(&TerminalColors::atom_one_light())
                        })
                }),
        }
    }

    pub(crate) fn prewarm_terminal_glyphs(&self) {
        prewarm_glyph_atlas(
            &self.glyph_atlas,
            &self.terminal_font,
            self.main_window_scale_factor,
        );
    }

    pub(crate) fn animate_titlebar_tabs(&mut self) {
        let manage_target = if self.active_tab == ActiveTab::Manage {
            TITLEBAR_MANAGE_ACTIVE_WIDTH
        } else {
            TITLEBAR_MANAGE_INACTIVE_WIDTH
        };

        self.manage_tab_width = animate_scalar(self.manage_tab_width, manage_target);

        for tab in &mut self.terminal_tabs {
            let target = if self.active_tab == ActiveTab::Terminal(tab.id) {
                TITLEBAR_TAB_ACTIVE_WIDTH
            } else {
                TITLEBAR_TAB_INACTIVE_WIDTH
            };

            tab.titlebar_width = animate_scalar(tab.titlebar_width, target);
        }
    }

    pub(crate) fn animate_sidebar_menu(&mut self) {
        for menu in ManageMenu::ALL {
            let target = if self.selected_menu == menu { 1.0 } else { 0.0 };
            let slot = &mut self.sidebar_menu_progress[menu.index()];

            if (*slot - target).abs() <= SIDEBAR_MENU_ANIMATION_EPSILON {
                *slot = target;
                continue;
            }

            *slot += (target - *slot) * SIDEBAR_MENU_ANIMATION_LERP;
        }
    }

    pub(crate) fn active_tab_index(&self) -> usize {
        match self.active_tab {
            ActiveTab::Manage => 0,
            ActiveTab::Terminal(id) => self
                .terminal_tabs
                .iter()
                .position(|tab| tab.id == id)
                .map(|index| index + 1)
                .unwrap_or(0),
        }
    }

    pub(crate) fn activate_tab_index(&mut self, index: usize) -> bool {
        if index == 0 {
            self.active_tab = ActiveTab::Manage;
            self.terminal_focus = None;
            return true;
        }

        let Some(tab) = self.terminal_tabs.get(index - 1) else {
            return false;
        };

        self.active_tab = ActiveTab::Terminal(tab.id);
        self.terminal_focus = Some(tab.id);
        true
    }

    pub(crate) fn activate_adjacent_tab(&mut self, direction: isize) -> bool {
        let total_tabs = self.terminal_tabs.len() + 1;
        if total_tabs <= 1 {
            return false;
        }

        let current = self.active_tab_index();
        let next = if direction.is_negative() {
            (current + total_tabs - 1) % total_tabs
        } else {
            (current + 1) % total_tabs
        };

        self.activate_tab_index(next)
    }

    pub(crate) fn close_terminal_tab(&mut self, id: u64) {
        if let Some(index) = self.terminal_tabs.iter().position(|tab| tab.id == id) {
            if let Some(session) = &self.terminal_tabs[index].session {
                let _ = session
                    .command_tx
                    .send(SessionCommand::Disconnect("标签页关闭".into()));
            }

            self.terminal_tabs.remove(index);

            if self.terminal_tabs.is_empty() {
                self.active_tab = ActiveTab::Manage;
                self.terminal_focus = None;
                return;
            }

            if self.active_tab == ActiveTab::Terminal(id) {
                let next_index = index.min(self.terminal_tabs.len().saturating_sub(1));
                let next_id = self.terminal_tabs[next_index].id;
                self.active_tab = ActiveTab::Terminal(next_id);
                self.terminal_focus = Some(next_id);
            } else if self.terminal_focus == Some(id) {
                self.terminal_focus = self.terminal_tabs.last().map(|tab| tab.id);
            }
        }
    }

    pub(crate) fn close_active_terminal_tab(&mut self) -> bool {
        let ActiveTab::Terminal(id) = self.active_tab else {
            return false;
        };

        self.close_terminal_tab(id);
        true
    }

    pub(crate) fn terminal_dimensions(&self) -> (usize, usize) {
        let font = self.terminal_font();
        let cell_width = font.metrics.cell_width;
        let cell_height = font.metrics.cell_height;
        let available_height = (self.main_window_size.height
            - TITLEBAR_HEIGHT
            - self.active_terminal_composer_total_height())
        .max(120.0);
        let available_width = self.main_window_size.width.max(240.0);

        let cols = (available_width / cell_width).floor() as usize;
        let rows = (available_height / cell_height).floor() as usize;

        (cols.max(20), rows.max(6))
    }

    pub(crate) fn terminal_composer_editor_height(&self, line_count: usize) -> f32 {
        let visible_lines =
            line_count.clamp(TERMINAL_COMPOSER_MIN_LINES, TERMINAL_COMPOSER_MAX_LINES) as f32;

        visible_lines * TERMINAL_COMPOSER_LINE_HEIGHT + TERMINAL_COMPOSER_INNER_PADDING_Y * 2.0
    }

    pub(crate) fn terminal_composer_total_height(&self, line_count: usize) -> f32 {
        self.terminal_composer_editor_height(line_count) + TERMINAL_COMPOSER_PADDING_Y * 2.0
    }

    pub(crate) fn active_terminal_composer_total_height(&self) -> f32 {
        match self.active_tab {
            ActiveTab::Manage => 0.0,
            ActiveTab::Terminal(id) => self
                .terminal_tabs
                .iter()
                .find(|tab| tab.id == id)
                .map(|tab| {
                    self.terminal_composer_total_height(
                        self.terminal_composer_visual_lines(&tab.composer.text()),
                    )
                })
                .unwrap_or_else(|| {
                    self.terminal_composer_total_height(TERMINAL_COMPOSER_MIN_LINES)
                }),
        }
    }

    pub(crate) fn terminal_composer_visual_lines(&self, text: &str) -> usize {
        let available_width = (self.main_window_size.width
            - TERMINAL_COMPOSER_HORIZONTAL_PADDING * 2.0
            - TERMINAL_COMPOSER_INNER_PADDING_X * 2.0)
            .max(120.0);

        text.lines()
            .map(|line| {
                let width = estimate_composer_text_width(line);
                ((width / available_width).ceil() as usize).max(1)
            })
            .sum::<usize>()
            .max(1)
    }

    pub(crate) fn resize_terminals(&mut self) {
        let (cols, rows) = self.terminal_dimensions();

        for tab in &mut self.terminal_tabs {
            tab.terminal.resize(cols, rows);
            if let Some(session) = &tab.session {
                let _ = session.command_tx.send(SessionCommand::Resize {
                    cols: cols as u16,
                    rows: rows as u16,
                });
            }
        }
    }

    pub(crate) fn open_terminal_from_connection(&mut self, connection_id: i64) -> Task<Message> {
        let Some(connection) = self
            .connections
            .iter()
            .find(|connection| connection.id == connection_id)
            .cloned()
        else {
            self.log(format!("未找到 connection #{connection_id}"));
            return Task::none();
        };

        let key = connection
            .effective_key_id
            .and_then(|id| self.keys.iter().find(|key| key.id == id))
            .cloned();
        let identity = connection
            .identity_id
            .and_then(|id| self.identities.iter().find(|identity| identity.id == id))
            .cloned();

        let (cols, rows) = self.terminal_dimensions();
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;

        let terminal = TerminalView::new(cols, rows, &self.settings.terminal);

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let target = ConnectionTarget {
            connection: connection.clone(),
            key,
            identity,
            known_hosts_path: self.paths.known_hosts.clone(),
            cols: cols as u16,
            rows: rows as u16,
        };

        let fallback_title = match connection.connection_type {
            ConnectionType::Local => local_terminal_tab_title(&connection),
            ConnectionType::Ssh | ConnectionType::Serial => connection.name.clone(),
        };

        self.terminal_tabs.push(TerminalTab {
            id: terminal_id,
            title: fallback_title.clone(),
            fallback_title,
            connection_type: connection.connection_type,
            theme_id: connection.theme_id.clone(),
            status: "Connecting".into(),
            titlebar_width: TITLEBAR_TAB_ACTIVE_WIDTH,
            workspace: TabWorkspace::Terminal,
            terminal,
            composer: text_editor::Content::new(),
            selection_anchor: None,
            selection: None,
            session: None,
        });
        self.active_tab = ActiveTab::Terminal(terminal_id);
        self.terminal_focus = Some(terminal_id);
        self.context_menu = None;
        self.log(format!("开始连接 connection: {}", connection.name));

        let event_stream = iced::futures::stream::unfold(event_rx, |mut rx| async move {
            rx.recv().await.map(|event| (event, rx))
        });

        Task::batch([
            Task::perform(connect_target(target, event_tx), move |result| {
                Message::TerminalConnected(terminal_id, result)
            }),
            Task::run(event_stream, move |event| {
                Message::TerminalSessionEvent(terminal_id, event)
            }),
        ])
    }

    pub(crate) fn open_sftp_from_connection(&mut self, connection_id: i64) -> Task<Message> {
        let Some(connection) = self
            .connections
            .iter()
            .find(|connection| {
                connection.id == connection_id && connection.connection_type == ConnectionType::Ssh
            })
            .cloned()
        else {
            self.log(format!("未找到可用的 SSH connection #{connection_id}"));
            return Task::none();
        };

        let key = connection
            .effective_key_id
            .and_then(|id| self.keys.iter().find(|key| key.id == id))
            .cloned();
        let identity = connection
            .identity_id
            .and_then(|id| self.identities.iter().find(|identity| identity.id == id))
            .cloned();

        let (cols, rows) = self.terminal_dimensions();
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;

        self.terminal_tabs.push(TerminalTab {
            id: terminal_id,
            title: format!("{} SFTP", connection.name),
            fallback_title: format!("{} SFTP", connection.name),
            connection_type: connection.connection_type,
            theme_id: "sftp".into(),
            status: "Connecting".into(),
            titlebar_width: TITLEBAR_TAB_ACTIVE_WIDTH,
            workspace: TabWorkspace::Sftp(SftpTabState {
                handle: None,
                current_path: ".".into(),
                entries: Vec::new(),
                selected_file: None,
                preview: String::new(),
                loading: true,
            }),
            terminal: TerminalView::new(cols, rows, &self.settings.terminal),
            composer: text_editor::Content::new(),
            selection_anchor: None,
            selection: None,
            session: None,
        });
        self.active_tab = ActiveTab::Terminal(terminal_id);
        self.terminal_focus = Some(terminal_id);
        self.context_menu = None;

        let target = ConnectionTarget {
            connection,
            key,
            identity,
            known_hosts_path: self.paths.known_hosts.clone(),
            cols: cols as u16,
            rows: rows as u16,
        };

        Task::perform(connect_sftp_target(target), move |result| {
            Message::SftpConnected(terminal_id, result)
        })
    }
}

fn estimate_composer_text_width(value: &str) -> f32 {
    value
        .chars()
        .map(|ch| estimated_composer_char_width(ch) * TERMINAL_COMPOSER_FONT_SIZE)
        .sum()
}

fn estimated_composer_char_width(ch: char) -> f32 {
    match ch {
        '0'..='9' => 0.56,
        'a'..='z' => 0.52,
        'A'..='Z' => 0.62,
        ' ' => 0.32,
        '-' | '_' | '.' | ':' | '/' | '\\' => 0.34,
        '@' | '#' | '&' | '%' => 0.68,
        '\u{4E00}'..='\u{9FFF}' => 1.0,
        _ if ch.is_ascii_punctuation() => 0.42,
        _ => 0.72,
    }
}

impl SettingsEditor {
    pub(crate) fn from_settings(settings: &AppSettings) -> Self {
        Self {
            font_family: settings.terminal.font.family.clone(),
            font_size: settings.terminal.font.size.to_string(),
            line_height: settings.terminal.font.line_height.to_string(),
            scrollback_lines: settings.terminal.scrollback_lines.to_string(),
            font_thicken: settings.terminal.font.thicken,
            cursor_shape: settings.terminal.cursor.shape.clone(),
            cursor_blinking: settings.terminal.cursor.blinking,
            background: settings.terminal.colors.primary.background.clone(),
            foreground: settings.terminal.colors.primary.foreground.clone(),
            cursor_color: settings.terminal.colors.cursor.cursor.clone(),
            cursor_text: settings.terminal.colors.cursor.text.clone(),
            selection_background: settings.terminal.colors.selection.background.clone(),
            selection_foreground: settings.terminal.colors.selection.text.clone(),
            ansi_normal: settings.terminal.colors.normal.as_array(),
            ansi_bright: settings.terminal.colors.bright.as_array(),
        }
    }

    pub(crate) fn apply_to_settings(&self, settings: &mut AppSettings) {
        settings.terminal.font.family = self.font_family.clone();
        if let Ok(size) = self.font_size.parse() {
            settings.terminal.font.size = size;
        }
        if let Ok(line_height) = self.line_height.parse() {
            settings.terminal.font.line_height = line_height;
        }
        if let Ok(scrollback_lines) = self.scrollback_lines.trim().parse() {
            settings.terminal.scrollback_lines = scrollback_lines;
        }
        settings.terminal.font.thicken = self.font_thicken;
        settings.terminal.cursor.shape = self.cursor_shape.clone();
        settings.terminal.cursor.blinking = self.cursor_blinking;
        settings.terminal.colors.primary.background = self.background.clone();
        settings.terminal.colors.primary.foreground = self.foreground.clone();
        settings.terminal.colors.cursor.cursor = self.cursor_color.clone();
        settings.terminal.colors.cursor.text = self.cursor_text.clone();
        settings.terminal.colors.selection.background = self.selection_background.clone();
        settings.terminal.colors.selection.text = self.selection_foreground.clone();
        settings.terminal.colors.normal = TerminalAnsiGroup::from_array(self.ansi_normal.clone());
        settings.terminal.colors.bright = TerminalAnsiGroup::from_array(self.ansi_bright.clone());
    }
}

impl ConnectionEditor {
    pub(crate) fn from_connection(connection: &Connection) -> Self {
        Self {
            id: Some(connection.id),
            name: connection.name.clone(),
            group_id: connection
                .group_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            key_id: connection
                .key_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".into()),
            identity_id: connection
                .identity_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".into()),
            host: connection.host.clone(),
            port: connection.port.to_string(),
            username: connection.username.clone(),
            password: connection.password.clone(),
            theme_id: connection.theme_id.clone(),
            shell_path: connection.shell_path.clone(),
            work_dir: connection.work_dir.clone(),
            startup_command: connection.startup_command.clone(),
            serial_port: connection.serial_port.clone(),
            baud_rate: connection.baud_rate.to_string(),
            connection_type: connection.connection_type,
        }
    }

    pub(crate) fn new() -> Self {
        Self::from_connection(&Connection::default())
    }

    pub(crate) fn to_connection(&self) -> Result<Connection, String> {
        let connection_type = self.connection_type;
        let host = if connection_type == ConnectionType::Ssh {
            self.host.trim().to_string()
        } else {
            String::new()
        };
        let port = if connection_type == ConnectionType::Ssh {
            self.port
                .trim()
                .parse::<i64>()
                .map_err(|_| "端口必须是数字".to_string())?
        } else {
            0
        };
        let serial_port = if connection_type == ConnectionType::Serial {
            self.serial_port.trim().to_string()
        } else {
            String::new()
        };
        let baud_rate = if connection_type == ConnectionType::Serial {
            self.baud_rate
                .trim()
                .parse::<i64>()
                .map_err(|_| "波特率必须是数字".to_string())?
        } else {
            115200
        };

        if self.name.trim().is_empty() {
            return Err("Connection 名称不能为空".into());
        }

        if connection_type == ConnectionType::Serial && serial_port.is_empty() {
            return Err("Serial 设备路径不能为空".into());
        }

        let shell_path = if connection_type == ConnectionType::Local {
            self.shell_path.trim().to_string()
        } else {
            String::new()
        };
        let work_dir = if connection_type == ConnectionType::Local {
            self.work_dir.trim().to_string()
        } else {
            String::new()
        };

        Ok(Connection {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            group_id: parse_optional_i64(&self.group_id),
            key_id: if connection_type == ConnectionType::Ssh {
                parse_optional_i64(&self.key_id)
            } else {
                None
            },
            effective_key_id: if connection_type == ConnectionType::Ssh {
                parse_optional_i64(&self.key_id)
            } else {
                None
            },
            identity_id: if connection_type == ConnectionType::Ssh {
                parse_optional_i64(&self.identity_id)
            } else {
                None
            },
            host,
            port,
            username: if connection_type == ConnectionType::Ssh {
                self.username.clone()
            } else {
                String::new()
            },
            display_username: if connection_type == ConnectionType::Ssh {
                self.username.clone()
            } else {
                String::new()
            },
            password: if connection_type == ConnectionType::Ssh {
                self.password.clone()
            } else {
                String::new()
            },
            theme_id: if self.theme_id.trim().is_empty() {
                "default".into()
            } else {
                self.theme_id.trim().to_string()
            },
            shell_path,
            work_dir,
            startup_command: self.startup_command.clone(),
            serial_port,
            baud_rate,
            connection_type,
        })
    }
}

impl KeyEditor {
    pub(crate) fn from_key(key: &SshKey) -> Self {
        Self {
            id: Some(key.id),
            name: key.name.clone(),
            private_key: key.private_key.clone(),
            private_key_content: text_editor::Content::with_text(&key.private_key),
            public_key: key.public_key.clone(),
            public_key_content: text_editor::Content::with_text(&key.public_key),
            certificate: key.certificate.clone(),
            certificate_content: text_editor::Content::with_text(&key.certificate),
        }
    }

    pub(crate) fn new() -> Self {
        Self::from_key(&SshKey::default())
    }

    pub(crate) fn to_key(&self) -> Result<SshKey, String> {
        if self.name.trim().is_empty() {
            return Err("Key 名称不能为空".into());
        }

        Ok(SshKey {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            private_key: self.private_key.clone(),
            public_key: self.public_key.clone(),
            certificate: self.certificate.clone(),
        })
    }
}

impl IdentityEditor {
    pub(crate) fn from_identity(identity: &Identity) -> Self {
        Self {
            id: Some(identity.id),
            name: identity.name.clone(),
            username: identity.username.clone(),
            password: identity.password.clone(),
            key_id: identity
                .key_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".into()),
        }
    }

    pub(crate) fn new() -> Self {
        Self::from_identity(&Identity::default())
    }

    pub(crate) fn to_identity(&self) -> Result<Identity, String> {
        if self.name.trim().is_empty() {
            return Err("Identity 名称不能为空".into());
        }

        Ok(Identity {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            username: self.username.clone(),
            password: self.password.clone(),
            key_id: parse_optional_i64(&self.key_id),
        })
    }
}

impl GroupEditor {
    pub(crate) fn from_group(group: &Group) -> Self {
        Self {
            id: Some(group.id),
            name: group.name.clone(),
            parent_id: group
                .parent_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".into()),
        }
    }

    pub(crate) fn new() -> Self {
        Self::from_group(&Group::default())
    }

    pub(crate) fn to_group(&self) -> Result<Group, String> {
        if self.name.trim().is_empty() {
            return Err("Group 名称不能为空".into());
        }

        Ok(Group {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            parent_id: parse_optional_i64(&self.parent_id),
        })
    }
}

impl PortForwardEditor {
    pub(crate) fn from_port_forward(forward: &PortForward) -> Self {
        Self {
            id: Some(forward.id),
            label: forward.label.clone(),
            forward_type: forward.forward_type,
            enabled: forward.enabled,
            bind_address: forward.bind_address.clone(),
            bind_port: forward.bind_port.to_string(),
            connection_id: forward
                .connection_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".into()),
            destination_host: forward.destination_host.clone(),
            destination_port: forward.destination_port.to_string(),
        }
    }

    pub(crate) fn new() -> Self {
        Self::from_port_forward(&PortForward::default())
    }

    pub(crate) fn to_port_forward(&self) -> Result<PortForward, String> {
        let bind_port = self
            .bind_port
            .trim()
            .parse::<i64>()
            .map_err(|_| "Bind Port 必须是数字".to_string())?;
        let destination_port = if self.forward_type == PortForwardType::Dynamic {
            0
        } else {
            self.destination_port
                .trim()
                .parse::<i64>()
                .map_err(|_| "Destination Port 必须是数字".to_string())?
        };

        Ok(PortForward {
            id: self.id.unwrap_or(0),
            label: self.label.trim().to_string(),
            forward_type: self.forward_type,
            enabled: self.enabled,
            bind_address: self.bind_address.trim().to_string(),
            bind_port,
            connection_id: parse_optional_i64(&self.connection_id),
            connection_name: String::new(),
            destination_host: self.destination_host.trim().to_string(),
            destination_port,
        })
    }
}
