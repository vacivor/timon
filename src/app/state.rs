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
    pub(crate) main_window: Option<window::Id>,
    pub(crate) settings_window: Option<window::Id>,
    pub(crate) main_window_size: iced::Size,
    pub(crate) main_window_vibrancy_installed: bool,
    pub(crate) selected_menu: ManageMenu,
    pub(crate) active_tab: ActiveTab,
    pub(crate) manage_tab_width: f32,
    pub(crate) terminal_tabs: Vec<TerminalTab>,
    pub(crate) next_terminal_id: u64,
    pub(crate) drawer: Option<DrawerState>,
    pub(crate) active_profile_context: Option<i64>,
    pub(crate) active_key_context: Option<i64>,
    pub(crate) active_identity_context: Option<i64>,
    pub(crate) terminal_focus: Option<u64>,
    pub(crate) terminal_composer_focus: Option<u64>,
    pub(crate) profiles: Vec<Profile>,
    pub(crate) keys: Vec<SshKey>,
    pub(crate) identities: Vec<Identity>,
    pub(crate) known_hosts: Vec<KnownHostEntry>,
    pub(crate) logs: Vec<String>,
    pub(crate) settings_editor: SettingsEditor,
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
    pub(crate) profile_type: ProfileType,
    pub(crate) theme_id: String,
    pub(crate) status: String,
    pub(crate) titlebar_width: f32,
    pub(crate) terminal: TerminalView,
    pub(crate) composer: text_editor::Content,
    pub(crate) selection_anchor: Option<TerminalPoint>,
    pub(crate) selection: Option<TerminalSelection>,
    pub(crate) session: Option<SessionHandle>,
    pub(crate) event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<SessionEvent>>,
}

#[derive(Debug, Clone)]
pub(crate) enum DrawerState {
    Profile(ProfileEditor),
    Key(KeyEditor),
    Identity(IdentityEditor),
}

#[derive(Debug, Clone)]
pub(crate) struct ProfileEditor {
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
    pub(crate) profile_type: ProfileType,
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
pub(crate) enum ProfileField {
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
    TerminalComposerAction(u64, text_editor::Action),
    SubmitTerminalComposer(u64),
    OpenProfileContext(i64),
    OpenKeyContext(i64),
    OpenIdentityContext(i64),
    DuplicateProfile(i64),
    NewProfile,
    EditProfile(i64),
    SaveProfile,
    ProfileFieldChanged(ProfileField, String),
    ProfileTypeChanged(String),
    NewKey,
    EditKey(i64),
    SaveKey,
    KeyFieldChanged(KeyField, String),
    KeyEditorAction(KeyTextField, text_editor::Action),
    NewIdentity,
    EditIdentity(i64),
    SaveIdentity,
    IdentityFieldChanged(IdentityField, String),
    CloseDrawer,
    ConnectProfile(i64),
    TerminalConnected(u64, Result<SessionHandle, String>),
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
        let mut settings = settings;
        settings.terminal.font.family =
            normalize_font_family_choice(&settings.terminal.font.family, &available_fonts);
        let terminal_font = TerminalFont::from_settings(&settings.terminal.font);
        let glyph_atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
        let settings_editor = SettingsEditor::from_settings(&settings);
        let profiles = database.list_profiles().unwrap_or_default();
        let keys = database.list_keys().unwrap_or_default();
        let identities = database.list_identities().unwrap_or_default();
        let known_hosts = read_known_hosts(&paths.known_hosts).unwrap_or_default();

        Self {
            paths,
            database,
            settings,
            terminal_font,
            glyph_atlas,
            main_window_scale_factor: 1.0,
            available_fonts,
            available_shells,
            main_window: None,
            settings_window: None,
            main_window_size: iced::Size::new(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT),
            main_window_vibrancy_installed: false,
            selected_menu: ManageMenu::Profiles,
            active_tab: ActiveTab::Manage,
            manage_tab_width: TITLEBAR_MANAGE_ACTIVE_WIDTH,
            terminal_tabs: Vec::new(),
            next_terminal_id: 1,
            drawer: None,
            active_profile_context: None,
            active_key_context: None,
            active_identity_context: None,
            terminal_focus: None,
            terminal_composer_focus: None,
            profiles,
            keys,
            identities,
            known_hosts,
            logs: vec!["Timon 已启动".into()],
            settings_editor,
        }
    }

    pub(crate) fn log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    pub(crate) fn reload_data(&mut self) {
        self.profiles = self.database.list_profiles().unwrap_or_default();
        self.keys = self.database.list_keys().unwrap_or_default();
        self.identities = self.database.list_identities().unwrap_or_default();
        self.known_hosts = read_known_hosts(&self.paths.known_hosts).unwrap_or_default();
    }

    pub(crate) fn terminal_font(&self) -> &TerminalFont {
        &self.terminal_font
    }

    pub(crate) fn terminal_theme(&self, theme_id: &str) -> TerminalTheme {
        let settings_theme = TerminalTheme::from_settings(&self.settings.terminal.colors);
        let atom = TerminalTheme::from_settings(&TerminalColors::atom_one_light());

        match theme_id {
            "default" => settings_theme,
            "atom-one-light" => atom,
            other if other == self.settings.terminal.default_theme_id => settings_theme,
            _ => atom,
        }
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

    pub(crate) fn open_terminal_from_profile(&mut self, profile_id: i64) -> Task<Message> {
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
        else {
            self.log(format!("未找到 profile #{profile_id}"));
            return Task::none();
        };

        let key = profile
            .effective_key_id
            .and_then(|id| self.keys.iter().find(|key| key.id == id))
            .cloned();
        let identity = profile
            .identity_id
            .and_then(|id| self.identities.iter().find(|identity| identity.id == id))
            .cloned();

        let (cols, rows) = self.terminal_dimensions();
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;

        let terminal = TerminalView::new(cols, rows, &self.settings.terminal);

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let target = ConnectionTarget {
            profile: profile.clone(),
            key,
            identity,
            known_hosts_path: self.paths.known_hosts.clone(),
            cols: cols as u16,
            rows: rows as u16,
        };

        let fallback_title = if profile.profile_type == ProfileType::Local {
            local_terminal_tab_title(&profile)
        } else {
            profile.name.clone()
        };

        self.terminal_tabs.push(TerminalTab {
            id: terminal_id,
            title: fallback_title.clone(),
            fallback_title,
            profile_type: profile.profile_type,
            theme_id: profile.theme_id.clone(),
            status: "Connecting".into(),
            titlebar_width: TITLEBAR_TAB_ACTIVE_WIDTH,
            terminal,
            composer: text_editor::Content::new(),
            selection_anchor: None,
            selection: None,
            session: None,
            event_rx: Some(event_rx),
        });
        self.active_tab = ActiveTab::Terminal(terminal_id);
        self.terminal_focus = Some(terminal_id);
        self.active_profile_context = None;
        self.log(format!("开始连接 profile: {}", profile.name));

        Task::perform(connect_target(target, event_tx), move |result| {
            Message::TerminalConnected(terminal_id, result)
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
            background: settings.terminal.colors.background.clone(),
            foreground: settings.terminal.colors.foreground.clone(),
            cursor_color: settings.terminal.colors.cursor_color.clone(),
            cursor_text: settings.terminal.colors.cursor_text.clone(),
            selection_background: settings.terminal.colors.selection_background.clone(),
            selection_foreground: settings.terminal.colors.selection_foreground.clone(),
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
        settings.terminal.colors.background = self.background.clone();
        settings.terminal.colors.foreground = self.foreground.clone();
        settings.terminal.colors.cursor_color = self.cursor_color.clone();
        settings.terminal.colors.cursor_text = self.cursor_text.clone();
        settings.terminal.colors.selection_background = self.selection_background.clone();
        settings.terminal.colors.selection_foreground = self.selection_foreground.clone();
        settings.terminal.colors.normal = TerminalAnsiGroup::from_array(self.ansi_normal.clone());
        settings.terminal.colors.bright = TerminalAnsiGroup::from_array(self.ansi_bright.clone());
    }
}

impl ProfileEditor {
    pub(crate) fn from_profile(profile: &Profile) -> Self {
        Self {
            id: Some(profile.id),
            name: profile.name.clone(),
            group_id: profile
                .group_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            key_id: profile
                .key_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".into()),
            identity_id: profile
                .identity_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "None".into()),
            host: profile.host.clone(),
            port: profile.port.to_string(),
            username: profile.username.clone(),
            password: profile.password.clone(),
            theme_id: profile.theme_id.clone(),
            shell_path: profile.shell_path.clone(),
            work_dir: profile.work_dir.clone(),
            startup_command: profile.startup_command.clone(),
            profile_type: profile.profile_type,
        }
    }

    pub(crate) fn new() -> Self {
        Self::from_profile(&Profile::default())
    }

    pub(crate) fn to_profile(&self) -> Result<Profile, String> {
        let profile_type = self.profile_type;
        let host = self.host.trim().to_string();
        let port = if profile_type == ProfileType::Local {
            0
        } else {
            self.port
                .trim()
                .parse::<i64>()
                .map_err(|_| "端口必须是数字".to_string())?
        };

        if self.name.trim().is_empty() {
            return Err("Profile 名称不能为空".into());
        }

        let shell_path = if profile_type == ProfileType::Local {
            self.shell_path.trim().to_string()
        } else {
            String::new()
        };
        let work_dir = if profile_type == ProfileType::Local {
            self.work_dir.trim().to_string()
        } else {
            String::new()
        };

        Ok(Profile {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            group_id: parse_optional_i64(&self.group_id),
            key_id: parse_optional_i64(&self.key_id),
            effective_key_id: parse_optional_i64(&self.key_id),
            identity_id: parse_optional_i64(&self.identity_id),
            host,
            port,
            username: self.username.clone(),
            display_username: self.username.clone(),
            password: self.password.clone(),
            theme_id: if self.theme_id.trim().is_empty() {
                "default".into()
            } else {
                self.theme_id.trim().to_string()
            },
            shell_path,
            work_dir,
            startup_command: self.startup_command.clone(),
            profile_type,
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
