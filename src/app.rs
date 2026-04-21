use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::alignment::{Horizontal, Vertical};
use iced::event;
use iced::keyboard;
use iced::keyboard::Key;
use iced::theme::Theme;
use iced::time;
use iced::widget::{
    Space, button, column, container, mouse_area, pick_list, row, rule, scrollable, text,
    text_input,
};
use iced::window;
use iced::{Color, Element, Length, Subscription, Task, border};

use crate::models::{Certificate, Identity, KnownHostEntry, ManageMenu, Profile, ProfileType};
use crate::persistence::{
    AppPaths, AppSettings, Database, TerminalColors, load_settings, read_known_hosts, save_settings,
};
use crate::session::{
    ConnectionTarget, SessionCommand, SessionEvent, SessionHandle, connect_target,
};
use crate::terminal::{
    GlyphAtlas, TerminalAtlas, TerminalCanvasEvent, TerminalFont, TerminalPoint, TerminalSelection,
    TerminalSnapshot, TerminalTheme, TerminalView, available_terminal_fonts,
    canonical_terminal_font_name,
};

const MAIN_WINDOW_WIDTH: f32 = 920.0;
const MAIN_WINDOW_HEIGHT: f32 = 620.0;
const SETTINGS_WINDOW_WIDTH: f32 = 760.0;
const SETTINGS_WINDOW_HEIGHT: f32 = 720.0;
const TITLEBAR_HEIGHT: f32 = 40.0;
const SIDEBAR_WIDTH: f32 = 210.0;
const DRAWER_WIDTH: f32 = 360.0;
const TICK_MS: u64 = 16;

pub fn run() -> iced::Result {
    iced::daemon(
        || {
            let paths = AppPaths::discover().expect("failed to discover app paths");
            let database = Database::new(&paths.database).expect("failed to init database");
            let settings = load_settings(&paths.settings).unwrap_or_default();
            let mut app = App::new(paths, database, settings);

            let (main_window, open_main) = window::open(main_window_settings());
            app.main_window = Some(main_window);

            (app, open_main.map(Message::MainWindowReady))
        },
        update,
        view,
    )
    .title(title)
    .subscription(subscription)
    .theme(theme)
    .style(style)
    .run()
}

pub struct App {
    paths: AppPaths,
    database: Database,
    settings: AppSettings,
    terminal_font: TerminalFont,
    glyph_atlas: Arc<Mutex<GlyphAtlas>>,
    main_window_scale_factor: f32,
    available_fonts: Vec<String>,
    main_window: Option<window::Id>,
    settings_window: Option<window::Id>,
    main_window_size: iced::Size,
    selected_menu: ManageMenu,
    active_tab: ActiveTab,
    terminal_tabs: Vec<TerminalTab>,
    next_terminal_id: u64,
    drawer: Option<DrawerState>,
    active_profile_context: Option<i64>,
    active_certificate_context: Option<i64>,
    active_identity_context: Option<i64>,
    terminal_focus: Option<u64>,
    profiles: Vec<Profile>,
    certificates: Vec<Certificate>,
    identities: Vec<Identity>,
    known_hosts: Vec<KnownHostEntry>,
    logs: Vec<String>,
    settings_editor: SettingsEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Manage,
    Terminal(u64),
}

struct TerminalTab {
    id: u64,
    title: String,
    theme_id: String,
    status: String,
    terminal: TerminalView,
    selection_anchor: Option<TerminalPoint>,
    selection: Option<TerminalSelection>,
    session: Option<SessionHandle>,
    event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<SessionEvent>>,
}

#[derive(Debug, Clone)]
enum DrawerState {
    Profile(ProfileEditor),
    Certificate(CertificateEditor),
    Identity(IdentityEditor),
}

#[derive(Debug, Clone)]
struct ProfileEditor {
    id: Option<i64>,
    name: String,
    group_id: String,
    certificate_id: String,
    identity_id: String,
    host: String,
    port: String,
    username: String,
    password: String,
    theme_id: String,
    startup_command: String,
    profile_type: ProfileType,
}

#[derive(Debug, Clone)]
struct CertificateEditor {
    id: Option<i64>,
    name: String,
    private_key: String,
    public_key: String,
}

#[derive(Debug, Clone)]
struct IdentityEditor {
    id: Option<i64>,
    name: String,
    username: String,
    password: String,
    certificate_id: String,
}

#[derive(Debug, Clone)]
struct SettingsEditor {
    font_family: String,
    font_size: String,
    line_height: String,
    font_thicken: bool,
    cursor_shape: String,
    cursor_blinking: bool,
    background: String,
    foreground: String,
    cursor: String,
    ansi_colors: [String; 16],
}

#[derive(Debug, Clone, Copy)]
enum ProfileField {
    Name,
    GroupId,
    CertificateId,
    IdentityId,
    Host,
    Port,
    Username,
    Password,
    ThemeId,
    StartupCommand,
}

#[derive(Debug, Clone, Copy)]
enum CertificateField {
    Name,
    PrivateKey,
    PublicKey,
}

#[derive(Debug, Clone, Copy)]
enum IdentityField {
    Name,
    Username,
    Password,
    CertificateId,
}

#[derive(Debug, Clone, Copy)]
enum FontField {
    Family,
    Size,
    LineHeight,
}

#[derive(Debug, Clone, Copy)]
enum CursorField {
    Shape,
}

#[derive(Debug, Clone, Copy)]
enum ColorField {
    Background,
    Foreground,
    Cursor,
    Ansi(usize),
}

#[derive(Debug, Clone)]
enum Message {
    MainWindowReady(window::Id),
    SettingsWindowReady(window::Id),
    Tick,
    WindowEvent(window::Id, window::Event),
    KeyboardInput(window::Id, keyboard::Event),
    DragWindow(window::Id),
    SelectMenu(ManageMenu),
    OpenSettingsWindow,
    ActivateManageTab,
    ActivateTerminal(u64),
    CloseTerminal(u64),
    TerminalSelectionStarted(u64, TerminalPoint),
    TerminalSelectionUpdated(u64, TerminalPoint),
    OpenProfileContext(i64),
    OpenCertificateContext(i64),
    OpenIdentityContext(i64),
    NewProfile,
    EditProfile(i64),
    SaveProfile,
    ProfileFieldChanged(ProfileField, String),
    ProfileTypeChanged(String),
    NewCertificate,
    EditCertificate(i64),
    SaveCertificate,
    CertificateFieldChanged(CertificateField, String),
    NewIdentity,
    EditIdentity(i64),
    SaveIdentity,
    IdentityFieldChanged(IdentityField, String),
    CloseDrawer,
    ConnectProfile(i64),
    TerminalConnected(u64, Result<SessionHandle, String>),
    TerminalDisconnect(u64),
    SettingsFontChanged(FontField, String),
    SettingsFontThickenChanged(bool),
    SettingsCursorChanged(CursorField, String),
    SettingsCursorBlinkChanged(bool),
    SettingsColorChanged(ColorField, String),
    SaveSettings,
    ResetThemeToAtomOneLight,
}

impl App {
    fn new(paths: AppPaths, database: Database, settings: AppSettings) -> Self {
        let available_fonts = available_terminal_fonts();
        let mut settings = settings;
        settings.terminal.font.family =
            normalize_font_family_choice(&settings.terminal.font.family, &available_fonts);
        let terminal_font = TerminalFont::from_settings(&settings.terminal.font);
        let glyph_atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
        let settings_editor = SettingsEditor::from_settings(&settings);
        let profiles = database.list_profiles().unwrap_or_default();
        let certificates = database.list_certificates().unwrap_or_default();
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
            main_window: None,
            settings_window: None,
            main_window_size: iced::Size::new(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT),
            selected_menu: ManageMenu::Profiles,
            active_tab: ActiveTab::Manage,
            terminal_tabs: Vec::new(),
            next_terminal_id: 1,
            drawer: None,
            active_profile_context: None,
            active_certificate_context: None,
            active_identity_context: None,
            terminal_focus: None,
            profiles,
            certificates,
            identities,
            known_hosts,
            logs: vec!["Timon 已启动".into()],
            settings_editor,
        }
    }

    fn log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    fn reload_data(&mut self) {
        self.profiles = self.database.list_profiles().unwrap_or_default();
        self.certificates = self.database.list_certificates().unwrap_or_default();
        self.identities = self.database.list_identities().unwrap_or_default();
        self.known_hosts = read_known_hosts(&self.paths.known_hosts).unwrap_or_default();
    }

    fn terminal_font(&self) -> &TerminalFont {
        &self.terminal_font
    }

    fn terminal_theme(&self, theme_id: &str) -> TerminalTheme {
        let settings_theme = TerminalTheme::from_settings(&self.settings.terminal.colors);
        let atom = TerminalTheme::from_settings(&TerminalColors::atom_one_light());

        match theme_id {
            "default" => settings_theme,
            "atom-one-light" => atom,
            other if other == self.settings.terminal.default_theme_id => settings_theme,
            _ => atom,
        }
    }

    fn current_terminal(&self) -> Option<&TerminalTab> {
        let ActiveTab::Terminal(id) = self.active_tab else {
            return None;
        };
        self.terminal_tabs.iter().find(|tab| tab.id == id)
    }

    fn terminal_dimensions(&self) -> (usize, usize) {
        let font = self.terminal_font();
        let cell_width = font.metrics.cell_width;
        let cell_height = font.metrics.cell_height;
        let available_height = (self.main_window_size.height - TITLEBAR_HEIGHT).max(120.0);
        let available_width = self.main_window_size.width.max(240.0);

        let cols = (available_width / cell_width).floor() as usize;
        let rows = (available_height / cell_height).floor() as usize;

        (cols.max(20), rows.max(6))
    }

    fn resize_terminals(&mut self) {
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

    fn open_terminal_from_profile(&mut self, profile_id: i64) -> Task<Message> {
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
        else {
            self.log(format!("未找到 profile #{profile_id}"));
            return Task::none();
        };

        let certificate = profile
            .certificate_id
            .and_then(|id| {
                self.certificates
                    .iter()
                    .find(|certificate| certificate.id == id)
            })
            .cloned();
        let identity = profile
            .identity_id
            .and_then(|id| self.identities.iter().find(|identity| identity.id == id))
            .cloned();

        let (cols, rows) = self.terminal_dimensions();
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;

        let terminal = TerminalView::new(cols, rows, &self.settings.terminal.cursor);

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let target = ConnectionTarget {
            profile: profile.clone(),
            certificate,
            identity,
            known_hosts_path: self.paths.known_hosts.clone(),
            cols: cols as u16,
            rows: rows as u16,
        };

        self.terminal_tabs.push(TerminalTab {
            id: terminal_id,
            title: profile.name.clone(),
            theme_id: profile.theme_id.clone(),
            status: "Connecting".into(),
            terminal,
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

impl SettingsEditor {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            font_family: settings.terminal.font.family.clone(),
            font_size: settings.terminal.font.size.to_string(),
            line_height: settings.terminal.font.line_height.to_string(),
            font_thicken: settings.terminal.font.thicken,
            cursor_shape: settings.terminal.cursor.shape.clone(),
            cursor_blinking: settings.terminal.cursor.blinking,
            background: settings.terminal.colors.background.clone(),
            foreground: settings.terminal.colors.foreground.clone(),
            cursor: settings.terminal.colors.cursor.clone(),
            ansi_colors: settings.terminal.colors.ansi_colors.clone(),
        }
    }

    fn apply_to_settings(&self, settings: &mut AppSettings) {
        settings.terminal.font.family = self.font_family.clone();
        if let Ok(size) = self.font_size.parse() {
            settings.terminal.font.size = size;
        }
        if let Ok(line_height) = self.line_height.parse() {
            settings.terminal.font.line_height = line_height;
        }
        settings.terminal.font.thicken = self.font_thicken;
        settings.terminal.cursor.shape = self.cursor_shape.clone();
        settings.terminal.cursor.blinking = self.cursor_blinking;
        settings.terminal.colors.background = self.background.clone();
        settings.terminal.colors.foreground = self.foreground.clone();
        settings.terminal.colors.cursor = self.cursor.clone();
        settings.terminal.colors.ansi_colors = self.ansi_colors.clone();
    }
}

impl ProfileEditor {
    fn from_profile(profile: &Profile) -> Self {
        Self {
            id: Some(profile.id),
            name: profile.name.clone(),
            group_id: profile
                .group_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            certificate_id: profile
                .certificate_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            identity_id: profile
                .identity_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            host: profile.host.clone(),
            port: profile.port.to_string(),
            username: profile.username.clone(),
            password: profile.password.clone(),
            theme_id: profile.theme_id.clone(),
            startup_command: profile.startup_command.clone(),
            profile_type: profile.profile_type,
        }
    }

    fn new() -> Self {
        Self::from_profile(&Profile::default())
    }

    fn to_profile(&self) -> Result<Profile, String> {
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

        Ok(Profile {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            group_id: parse_optional_i64(&self.group_id),
            certificate_id: parse_optional_i64(&self.certificate_id),
            identity_id: parse_optional_i64(&self.identity_id),
            host,
            port,
            username: self.username.clone(),
            password: self.password.clone(),
            theme_id: if self.theme_id.trim().is_empty() {
                "default".into()
            } else {
                self.theme_id.trim().to_string()
            },
            startup_command: self.startup_command.clone(),
            profile_type,
        })
    }
}

impl CertificateEditor {
    fn from_certificate(certificate: &Certificate) -> Self {
        Self {
            id: Some(certificate.id),
            name: certificate.name.clone(),
            private_key: certificate.private_key.clone(),
            public_key: certificate.public_key.clone(),
        }
    }

    fn new() -> Self {
        Self::from_certificate(&Certificate::default())
    }

    fn to_certificate(&self) -> Result<Certificate, String> {
        if self.name.trim().is_empty() {
            return Err("Certificate 名称不能为空".into());
        }

        Ok(Certificate {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            private_key: self.private_key.clone(),
            public_key: self.public_key.clone(),
        })
    }
}

impl IdentityEditor {
    fn from_identity(identity: &Identity) -> Self {
        Self {
            id: Some(identity.id),
            name: identity.name.clone(),
            username: identity.username.clone(),
            password: identity.password.clone(),
            certificate_id: identity
                .certificate_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
        }
    }

    fn new() -> Self {
        Self::from_identity(&Identity::default())
    }

    fn to_identity(&self) -> Result<Identity, String> {
        if self.name.trim().is_empty() {
            return Err("Identity 名称不能为空".into());
        }

        Ok(Identity {
            id: self.id.unwrap_or(0),
            name: self.name.trim().to_string(),
            username: self.username.clone(),
            password: self.password.clone(),
            certificate_id: parse_optional_i64(&self.certificate_id),
        })
    }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::MainWindowReady(id) => {
            app.main_window = Some(id);
            app.resize_terminals();
        }
        Message::SettingsWindowReady(id) => {
            app.settings_window = Some(id);
        }
        Message::Tick => {
            let mut log_messages = Vec::new();

            for tab in &mut app.terminal_tabs {
                let mut pending = Vec::new();
                if let Some(rx) = &mut tab.event_rx {
                    while let Ok(event) = rx.try_recv() {
                        pending.push(event);
                    }
                }

                for event in pending {
                    match event {
                        SessionEvent::Connected { description } => {
                            tab.status = description.clone();
                            log_messages.push(format!("{}: {}", tab.title, description));
                        }
                        SessionEvent::Output(bytes) => tab.terminal.feed(&bytes),
                        SessionEvent::Status(status) => {
                            tab.status = status.clone();
                            log_messages.push(format!("{}: {}", tab.title, status));
                        }
                        SessionEvent::Error(error) => {
                            tab.status = error.clone();
                            tab.terminal.push_local_line(&format!("Error: {error}"));
                            log_messages.push(format!("{}: {error}", tab.title));
                        }
                        SessionEvent::Disconnected(reason) => {
                            tab.status = reason.clone();
                            tab.terminal
                                .push_local_line(&format!("Disconnected: {reason}"));
                            tab.session = None;
                            log_messages.push(format!("{}: {reason}", tab.title));
                        }
                    }
                }
            }

            for log in log_messages {
                app.log(log);
            }
        }
        Message::WindowEvent(id, event) => match event {
            window::Event::Opened { size, .. } => {
                if Some(id) == app.main_window {
                    app.main_window_size = size;
                    app.resize_terminals();
                }
            }
            window::Event::Resized(size) => {
                if Some(id) == app.main_window {
                    app.main_window_size = size;
                    app.resize_terminals();
                }
            }
            window::Event::Rescaled(scale_factor) => {
                if Some(id) == app.main_window {
                    app.main_window_scale_factor = scale_factor.max(1.0);
                    app.glyph_atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
                }
            }
            window::Event::Closed => {
                if Some(id) == app.settings_window {
                    app.settings_window = None;
                }
                if Some(id) == app.main_window {
                    return iced::exit();
                }
            }
            _ => {}
        },
        Message::KeyboardInput(
            id,
            keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            },
        ) => {
            if Some(id) == app.main_window {
                if is_copy_shortcut(&key, modifiers) {
                    if let Some(tab_id) = app.terminal_focus {
                        if let Some(tab) = app.terminal_tabs.iter().find(|tab| tab.id == tab_id) {
                            let snapshot =
                                tab.terminal.snapshot(&app.terminal_theme(&tab.theme_id));
                            if let Some(contents) =
                                selection_contents(&snapshot, tab.selection.as_ref())
                            {
                                return iced::clipboard::write(contents);
                            }
                        }
                    }
                    return Task::none();
                }

                if let Some(tab_id) = app.terminal_focus {
                    if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                        if let Some(bytes) = tab.terminal.encode_key(
                            key,
                            modifiers,
                            text.map(|value| value.to_string()),
                        ) {
                            if let Some(session) = &tab.session {
                                let _ = session.command_tx.send(SessionCommand::Input(bytes));
                            }
                        }
                    }
                }
            }
        }
        Message::KeyboardInput(_, _) => {}
        Message::DragWindow(id) => return window::drag(id),
        Message::SelectMenu(menu) => {
            app.selected_menu = menu;
            app.active_profile_context = None;
            app.active_certificate_context = None;
            app.active_identity_context = None;

            if menu == ManageMenu::Settings {
                return open_settings_window(app);
            }
        }
        Message::OpenSettingsWindow => return open_settings_window(app),
        Message::ActivateManageTab => {
            app.active_tab = ActiveTab::Manage;
            app.terminal_focus = None;
        }
        Message::ActivateTerminal(id) => {
            app.active_tab = ActiveTab::Terminal(id);
            app.terminal_focus = Some(id);
        }
        Message::CloseTerminal(id) => {
            if let Some(index) = app.terminal_tabs.iter().position(|tab| tab.id == id) {
                if let Some(session) = &app.terminal_tabs[index].session {
                    let _ = session
                        .command_tx
                        .send(SessionCommand::Disconnect("标签页关闭".into()));
                }
                app.terminal_tabs.remove(index);
                if app.active_tab == ActiveTab::Terminal(id) {
                    app.active_tab = ActiveTab::Manage;
                    app.terminal_focus = None;
                }
            }
        }
        Message::TerminalSelectionStarted(id, point) => {
            app.active_tab = ActiveTab::Terminal(id);
            app.terminal_focus = Some(id);

            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                tab.selection_anchor = Some(point);
                tab.selection = Some(TerminalSelection {
                    start: point,
                    end: point,
                });
            }
        }
        Message::TerminalSelectionUpdated(id, point) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                let anchor = tab.selection_anchor.unwrap_or(point);
                tab.selection_anchor = Some(anchor);
                tab.selection = Some(normalize_selection(anchor, point));
            }
        }
        Message::OpenProfileContext(id) => {
            app.active_profile_context = Some(id);
            app.active_certificate_context = None;
            app.active_identity_context = None;
        }
        Message::OpenCertificateContext(id) => {
            app.active_profile_context = None;
            app.active_certificate_context = Some(id);
            app.active_identity_context = None;
        }
        Message::OpenIdentityContext(id) => {
            app.active_profile_context = None;
            app.active_certificate_context = None;
            app.active_identity_context = Some(id);
        }
        Message::NewProfile => {
            app.drawer = Some(DrawerState::Profile(ProfileEditor::new()));
            app.active_profile_context = None;
        }
        Message::EditProfile(id) => {
            if let Some(profile) = app.profiles.iter().find(|profile| profile.id == id) {
                app.drawer = Some(DrawerState::Profile(ProfileEditor::from_profile(profile)));
                app.active_profile_context = None;
            }
        }
        Message::ProfileFieldChanged(field, value) => {
            if let Some(DrawerState::Profile(editor)) = &mut app.drawer {
                match field {
                    ProfileField::Name => editor.name = value,
                    ProfileField::GroupId => editor.group_id = value,
                    ProfileField::CertificateId => editor.certificate_id = value,
                    ProfileField::IdentityId => editor.identity_id = value,
                    ProfileField::Host => editor.host = value,
                    ProfileField::Port => editor.port = value,
                    ProfileField::Username => editor.username = value,
                    ProfileField::Password => editor.password = value,
                    ProfileField::ThemeId => editor.theme_id = value,
                    ProfileField::StartupCommand => editor.startup_command = value,
                }
            }
        }
        Message::ProfileTypeChanged(value) => {
            if let Some(DrawerState::Profile(editor)) = &mut app.drawer {
                editor.profile_type = ProfileType::from(value.as_str());
            }
        }
        Message::SaveProfile => {
            if let Some(DrawerState::Profile(editor)) = &app.drawer {
                match editor.to_profile() {
                    Ok(mut profile) => match app.database.save_profile(&mut profile) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 profile: {}", profile.name));
                        }
                        Err(error) => app.log(format!("保存 profile 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::NewCertificate => {
            app.drawer = Some(DrawerState::Certificate(CertificateEditor::new()));
            app.active_certificate_context = None;
        }
        Message::EditCertificate(id) => {
            if let Some(certificate) = app
                .certificates
                .iter()
                .find(|certificate| certificate.id == id)
            {
                app.drawer = Some(DrawerState::Certificate(
                    CertificateEditor::from_certificate(certificate),
                ));
                app.active_certificate_context = None;
            }
        }
        Message::CertificateFieldChanged(field, value) => {
            if let Some(DrawerState::Certificate(editor)) = &mut app.drawer {
                match field {
                    CertificateField::Name => editor.name = value,
                    CertificateField::PrivateKey => editor.private_key = value,
                    CertificateField::PublicKey => editor.public_key = value,
                }
            }
        }
        Message::SaveCertificate => {
            if let Some(DrawerState::Certificate(editor)) = &app.drawer {
                match editor.to_certificate() {
                    Ok(mut certificate) => match app.database.save_certificate(&mut certificate) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 certificate: {}", certificate.name));
                        }
                        Err(error) => app.log(format!("保存 certificate 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::NewIdentity => {
            app.drawer = Some(DrawerState::Identity(IdentityEditor::new()));
            app.active_identity_context = None;
        }
        Message::EditIdentity(id) => {
            if let Some(identity) = app.identities.iter().find(|identity| identity.id == id) {
                app.drawer = Some(DrawerState::Identity(IdentityEditor::from_identity(
                    identity,
                )));
                app.active_identity_context = None;
            }
        }
        Message::IdentityFieldChanged(field, value) => {
            if let Some(DrawerState::Identity(editor)) = &mut app.drawer {
                match field {
                    IdentityField::Name => editor.name = value,
                    IdentityField::Username => editor.username = value,
                    IdentityField::Password => editor.password = value,
                    IdentityField::CertificateId => editor.certificate_id = value,
                }
            }
        }
        Message::SaveIdentity => {
            if let Some(DrawerState::Identity(editor)) = &app.drawer {
                match editor.to_identity() {
                    Ok(mut identity) => match app.database.save_identity(&mut identity) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 identity: {}", identity.name));
                        }
                        Err(error) => app.log(format!("保存 identity 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::CloseDrawer => app.drawer = None,
        Message::ConnectProfile(id) => return app.open_terminal_from_profile(id),
        Message::TerminalConnected(id, result) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                match result {
                    Ok(handle) => {
                        tab.session = Some(handle);
                        tab.status = "Connected".into();
                    }
                    Err(error) => {
                        tab.status = error.clone();
                        tab.terminal
                            .push_local_line(&format!("Connection error: {error}"));
                        let title = tab.title.clone();
                        app.log(format!("{title}: {error}"));
                    }
                }
            }
        }
        Message::TerminalDisconnect(id) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                if let Some(session) = &tab.session {
                    let _ = session
                        .command_tx
                        .send(SessionCommand::Disconnect("用户主动断开".into()));
                }
                tab.session = None;
                tab.status = "Disconnected".into();
            }
        }
        Message::SettingsFontChanged(field, value) => match field {
            FontField::Family => app.settings_editor.font_family = value,
            FontField::Size => app.settings_editor.font_size = value,
            FontField::LineHeight => app.settings_editor.line_height = value,
        },
        Message::SettingsFontThickenChanged(value) => {
            app.settings_editor.font_thicken = value;
        }
        Message::SettingsCursorChanged(CursorField::Shape, value) => {
            app.settings_editor.cursor_shape = value;
        }
        Message::SettingsCursorBlinkChanged(value) => {
            app.settings_editor.cursor_blinking = value;
        }
        Message::SettingsColorChanged(field, value) => match field {
            ColorField::Background => app.settings_editor.background = value,
            ColorField::Foreground => app.settings_editor.foreground = value,
            ColorField::Cursor => app.settings_editor.cursor = value,
            ColorField::Ansi(index) => {
                if let Some(slot) = app.settings_editor.ansi_colors.get_mut(index) {
                    *slot = value;
                }
            }
        },
        Message::SaveSettings => {
            app.settings_editor.font_family = normalize_font_family_choice(
                &app.settings_editor.font_family,
                &app.available_fonts,
            );
            app.settings_editor.apply_to_settings(&mut app.settings);
            match save_settings(&app.paths.settings, &app.settings) {
                Ok(()) => {
                    let cursor = app.settings.terminal.cursor.clone();
                    app.terminal_font = TerminalFont::from_settings(&app.settings.terminal.font);
                    app.glyph_atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
                    for tab in &mut app.terminal_tabs {
                        tab.terminal.reset(&cursor);
                    }
                    app.log(format!("设置已保存到 {}", app.paths.settings.display()));
                    app.resize_terminals();
                }
                Err(error) => app.log(format!("保存设置失败: {error:#}")),
            }
        }
        Message::ResetThemeToAtomOneLight => {
            let colors = TerminalColors::atom_one_light();
            app.settings_editor.background = colors.background;
            app.settings_editor.foreground = colors.foreground;
            app.settings_editor.cursor = colors.cursor;
            app.settings_editor.ansi_colors = colors.ansi_colors;
        }
    }

    Task::none()
}

fn title(app: &App, window: window::Id) -> String {
    if Some(window) == app.settings_window {
        return "Timon Settings".into();
    }

    match app.active_tab {
        ActiveTab::Manage => "Timon".into(),
        ActiveTab::Terminal(id) => app
            .terminal_tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| format!("{} - Timon", tab.title))
            .unwrap_or_else(|| "Timon".into()),
    }
}

fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::batch([
        time::every(Duration::from_millis(TICK_MS)).map(|_| Message::Tick),
        window::events().map(|(id, event)| Message::WindowEvent(id, event)),
        event::listen_with(|event, status, window| match (status, event) {
            (event::Status::Ignored, iced::Event::Keyboard(key_event)) => {
                Some(Message::KeyboardInput(window, key_event))
            }
            _ => None,
        }),
    ])
}

fn theme(_app: &App, _window: window::Id) -> Theme {
    Theme::Light
}

fn style(_app: &App, _theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::from_rgb8(28, 30, 36),
    }
}

fn view(app: &App, window: window::Id) -> Element<'_, Message> {
    if Some(window) == app.settings_window {
        return settings_window_view(app).into();
    }

    main_window_view(app, window).into()
}

fn main_window_view<'a>(app: &'a App, window: window::Id) -> iced::widget::Container<'a, Message> {
    let top_bar = container(
        row![
            mac_titlebar_spacer(),
            button(text("Manage"))
                .style(if app.active_tab == ActiveTab::Manage {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::ActivateManageTab),
            terminal_tab_buttons(app),
            mouse_area(
                container(
                    Space::new()
                        .width(Length::Fill)
                        .height(Length::Fixed(TITLEBAR_HEIGHT))
                )
                .width(Length::Fill)
            )
            .on_press(Message::DragWindow(window)),
            titlebar_controls(app),
        ]
        .align_y(Vertical::Center)
        .spacing(8),
    )
    .padding([6, 10])
    .height(Length::Fixed(TITLEBAR_HEIGHT))
    .style(|_| panel_style(Color::from_rgba8(255, 255, 255, 1.0)));

    let body: Element<'_, _> = match app.active_tab {
        ActiveTab::Manage => manage_page_view(app).into(),
        ActiveTab::Terminal(id) => terminal_page_view(app, id),
    };

    container(
        column![top_bar, body]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
}

fn settings_window_view<'a>(app: &'a App) -> iced::widget::Container<'a, Message> {
    let settings = &app.settings_editor;
    let selected_font = app
        .available_fonts
        .iter()
        .find(|family| *family == &settings.font_family);
    let font_section = settings_section(
        "Font",
        column![
            labeled_pick_list(
                "Family",
                app.available_fonts.as_slice(),
                selected_font,
                "Select a terminal font",
                |value| Message::SettingsFontChanged(FontField::Family, value)
            ),
            labeled_input("Size", &settings.font_size, |value| {
                Message::SettingsFontChanged(FontField::Size, value)
            }),
            labeled_input("Line Height", &settings.line_height, |value| {
                Message::SettingsFontChanged(FontField::LineHeight, value)
            }),
            button(if settings.font_thicken {
                "Font Thicken: On"
            } else {
                "Font Thicken: Off"
            })
            .on_press(Message::SettingsFontThickenChanged(!settings.font_thicken,)),
        ]
        .spacing(10),
    );

    let cursor_section = settings_section(
        "Cursor",
        column![
            labeled_input(
                "Shape (block / beam / underline)",
                &settings.cursor_shape,
                |value| Message::SettingsCursorChanged(CursorField::Shape, value)
            ),
            button(if settings.cursor_blinking {
                "Blinking: On"
            } else {
                "Blinking: Off"
            })
            .on_press(Message::SettingsCursorBlinkChanged(
                !settings.cursor_blinking,
            )),
        ]
        .spacing(10),
    );

    let colors_section = settings_section(
        "ANSI Colors",
        column![
            labeled_input("Background", &settings.background, |value| {
                Message::SettingsColorChanged(ColorField::Background, value)
            }),
            labeled_input("Foreground", &settings.foreground, |value| {
                Message::SettingsColorChanged(ColorField::Foreground, value)
            }),
            labeled_input("Cursor", &settings.cursor, |value| {
                Message::SettingsColorChanged(ColorField::Cursor, value)
            }),
            ansi_color_inputs(&settings.ansi_colors),
        ]
        .spacing(10),
    );

    container(
        column![
            row![
                text("Settings").size(26),
                Space::new().width(Length::Fill),
                button("Reset to Atom One Light").on_press(Message::ResetThemeToAtomOneLight),
                button("Save").on_press(Message::SaveSettings),
            ]
            .align_y(Vertical::Center)
            .spacing(10),
            rule::horizontal(1),
            scrollable(
                column![font_section, cursor_section, colors_section]
                    .spacing(18)
                    .width(Length::Fill)
            )
            .height(Length::Fill),
        ]
        .spacing(14)
        .padding(20),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| panel_style(Color::from_rgb8(248, 250, 252)))
}

fn manage_page_view(app: &App) -> iced::widget::Container<'_, Message> {
    let sidebar = container(
        column![
            text("Management").size(22),
            rule::horizontal(1),
            menu_buttons(app),
        ]
        .spacing(12)
        .padding(18),
    )
    .width(Length::Fixed(SIDEBAR_WIDTH))
    .height(Length::Fill)
    .style(|_| panel_style(Color::from_rgb8(245, 247, 251)));

    let content_body: Element<'_, Message> = match app.selected_menu {
        ManageMenu::Profiles => profiles_view(app),
        ManageMenu::Keychain => keychain_view(app),
        ManageMenu::PortForwarding => placeholder_view(
            "Port Forwarding",
            "下一步可以把本地/远端端口转发规则也纳入 SQLite 管理。",
        )
        .into(),
        ManageMenu::Snippets => {
            placeholder_view("Snippets", "这里预留给快速命令和片段管理。").into()
        }
        ManageMenu::KnownHosts => known_hosts_view(app),
        ManageMenu::Logs => logs_view(app),
        ManageMenu::Settings => placeholder_view("Settings", "设置页会在新窗口中打开。").into(),
    };

    let content = container(content_body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(18)
        .style(|_| panel_style(Color::from_rgb8(255, 255, 255)));

    let row = if let Some(drawer) = &app.drawer {
        row![sidebar, content, drawer_view(drawer)]
    } else {
        row![sidebar, content]
    };

    container(row.height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
}

fn terminal_page_view(app: &App, id: u64) -> Element<'_, Message> {
    let Some(tab) = app.terminal_tabs.iter().find(|tab| tab.id == id) else {
        return placeholder_view("Terminal", "Tab not found").into();
    };

    let snapshot = tab.terminal.snapshot(&app.terminal_theme(&tab.theme_id));
    let terminal = TerminalAtlas::new(
        snapshot,
        tab.selection.clone(),
        app.terminal_font().clone(),
        app.glyph_atlas.clone(),
        app.main_window_scale_factor,
        Arc::new(move |event| match event {
            TerminalCanvasEvent::SelectionStarted(point) => {
                Message::TerminalSelectionStarted(id, point)
            }
            TerminalCanvasEvent::SelectionUpdated(point) => {
                Message::TerminalSelectionUpdated(id, point)
            }
        }),
    )
    .element();

    container(terminal)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn normalize_selection(anchor: TerminalPoint, head: TerminalPoint) -> TerminalSelection {
    if (anchor.line, anchor.column) <= (head.line, head.column) {
        TerminalSelection {
            start: anchor,
            end: head,
        }
    } else {
        TerminalSelection {
            start: head,
            end: anchor,
        }
    }
}

fn is_copy_shortcut(key: &Key, modifiers: keyboard::Modifiers) -> bool {
    matches!(key.as_ref(), Key::Character("c") | Key::Character("C"))
        && (modifiers.command() || (modifiers.control() && modifiers.shift()))
}

fn selection_contents(
    snapshot: &TerminalSnapshot,
    selection: Option<&TerminalSelection>,
) -> Option<String> {
    let selection = selection?;
    let mut rows = std::collections::BTreeMap::<usize, Vec<_>>::new();

    for cell in &snapshot.cells {
        if !cell_in_selection(selection, cell.line, cell.column, cell.width) {
            continue;
        }

        rows.entry(cell.line).or_default().push(cell);
    }

    if rows.is_empty() {
        return None;
    }

    let mut output = String::new();

    for (index, line) in (selection.start.line..=selection.end.line).enumerate() {
        let mut current_column = if line == selection.start.line {
            selection.start.column
        } else {
            0
        };
        let mut line_output = String::new();

        if let Some(cells) = rows.get(&line) {
            let mut cells = cells.clone();
            cells.sort_by_key(|cell| cell.column);

            for cell in cells {
                if cell.column > current_column {
                    line_output.push_str(&" ".repeat(cell.column - current_column));
                }

                line_output.push_str(&cell.text);
                current_column = cell.column + cell.width.max(1);
            }
        }

        while line_output.ends_with(' ') {
            line_output.pop();
        }

        if index > 0 {
            output.push('\n');
        }

        output.push_str(&line_output);

        if index + 1 == (selection.end.line - selection.start.line + 1) {
            break;
        }
    }

    Some(output)
}

fn cell_in_selection(
    selection: &TerminalSelection,
    line: usize,
    column: usize,
    width: usize,
) -> bool {
    let cell_start = (line, column);
    let cell_end = (line, column + width.saturating_sub(1));
    let selection_start = (selection.start.line, selection.start.column);
    let selection_end = (selection.end.line, selection.end.column);

    cell_end >= selection_start && cell_start <= selection_end
}

fn profiles_view(app: &App) -> Element<'_, Message> {
    let header = row![
        text("Profiles").size(28),
        Space::new().width(Length::Fill),
        button("New Profile").on_press(Message::NewProfile),
    ]
    .align_y(Vertical::Center);

    let cards = app
        .profiles
        .iter()
        .fold(column![header].spacing(14), |column, profile| {
            column.push(profile_card(app, profile))
        });

    scrollable(cards).height(Length::Fill).into()
}

fn keychain_view(app: &App) -> Element<'_, Message> {
    let certificates = app.certificates.iter().fold(
        column![
            row![
                text("Certificates").size(22),
                Space::new().width(Length::Fill),
                button("New Certificate").on_press(Message::NewCertificate),
            ]
            .align_y(Vertical::Center)
        ]
        .spacing(12),
        |column, certificate| column.push(certificate_card(app, certificate)),
    );

    let identities = app.identities.iter().fold(
        column![
            row![
                text("Identities").size(22),
                Space::new().width(Length::Fill),
                button("New Identity").on_press(Message::NewIdentity),
            ]
            .align_y(Vertical::Center)
        ]
        .spacing(12),
        |column, identity| column.push(identity_card(app, identity)),
    );

    scrollable(column![certificates, identities].spacing(24))
        .height(Length::Fill)
        .into()
}

fn known_hosts_view(app: &App) -> Element<'_, Message> {
    let list = if app.known_hosts.is_empty() {
        column![text("还没有记录 known hosts").size(15)]
    } else {
        app.known_hosts
            .iter()
            .fold(column![].spacing(8), |column, entry| {
                column.push(
                    container(
                        column![
                            text(format!("Line {}", entry.line_number)).size(13),
                            text(entry.line.clone())
                                .font(iced::Font::MONOSPACE)
                                .size(14),
                        ]
                        .spacing(4),
                    )
                    .padding(12)
                    .style(iced::widget::container::rounded_box),
                )
            })
    };

    scrollable(
        column![
            text("Known Hosts").size(28),
            text(app.paths.known_hosts.display().to_string())
                .size(14)
                .color(Color::from_rgb8(108, 117, 138)),
            rule::horizontal(1),
            list,
        ]
        .spacing(14),
    )
    .height(Length::Fill)
    .into()
}

fn logs_view(app: &App) -> Element<'_, Message> {
    let entries = app
        .logs
        .iter()
        .rev()
        .fold(column![].spacing(8), |column, line| {
            column.push(
                container(text(line.clone()).font(iced::Font::MONOSPACE).size(14))
                    .padding(10)
                    .style(iced::widget::container::rounded_box),
            )
        });

    scrollable(column![text("Logs").size(28), entries].spacing(12))
        .height(Length::Fill)
        .into()
}

fn placeholder_view<'a>(title: &'a str, body: &'a str) -> iced::widget::Container<'a, Message> {
    container(
        column![text(title).size(28), text(body).size(16)]
            .spacing(12)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Left),
    )
    .width(Length::Fill)
    .height(Length::Fill)
}

fn drawer_view(drawer: &DrawerState) -> iced::widget::Container<'_, Message> {
    let content: Element<'_, _> = match drawer {
        DrawerState::Profile(editor) => profile_drawer(editor).into(),
        DrawerState::Certificate(editor) => certificate_drawer(editor).into(),
        DrawerState::Identity(editor) => identity_drawer(editor).into(),
    };

    container(content)
        .width(Length::Fixed(DRAWER_WIDTH))
        .height(Length::Fill)
        .style(|_| panel_style(Color::from_rgb8(249, 250, 252)))
}

fn profile_card<'a>(app: &'a App, profile: &'a Profile) -> Element<'a, Message> {
    let type_label = if profile.profile_type == ProfileType::Local {
        "local"
    } else {
        "ssh"
    };

    let actions: Element<'a, Message> = if app.active_profile_context == Some(profile.id) {
        container(
            row![
                button("Connect").on_press(Message::ConnectProfile(profile.id)),
                button("Edit").on_press(Message::EditProfile(profile.id)),
            ]
            .spacing(8),
        )
        .into()
    } else {
        Space::new().height(Length::Shrink).into()
    };

    let body = column![
        row![
            text(profile.name.clone()).size(20),
            Space::new().width(Length::Fill),
            text(type_label)
                .size(13)
                .color(Color::from_rgb8(108, 117, 138)),
        ],
        text(if profile.profile_type == ProfileType::Local {
            "Local PTY terminal".to_string()
        } else {
            format!(
                "{}@{}:{}",
                empty_as_dash(&profile.username),
                empty_as_dash(&profile.host),
                profile.port
            )
        })
        .size(15),
        text(format!("theme_id: {}", empty_as_dash(&profile.theme_id)))
            .size(13)
            .color(Color::from_rgb8(108, 117, 138)),
        actions,
    ]
    .spacing(10);

    mouse_area(
        container(body)
            .padding(16)
            .style(iced::widget::container::rounded_box),
    )
    .on_right_press(Message::OpenProfileContext(profile.id))
    .into()
}

fn certificate_card<'a>(app: &'a App, certificate: &'a Certificate) -> Element<'a, Message> {
    let actions: Element<'a, Message> = if app.active_certificate_context == Some(certificate.id) {
        container(
            row![button("Edit").on_press(Message::EditCertificate(certificate.id))].spacing(8),
        )
        .into()
    } else {
        Space::new().height(Length::Shrink).into()
    };

    let body = column![
        row![
            text(certificate.name.clone()).size(18),
            Space::new().width(Length::Fill),
            text(format!("#{}", certificate.id))
                .size(13)
                .color(Color::from_rgb8(108, 117, 138)),
        ],
        text(short_preview(&certificate.public_key))
            .font(iced::Font::MONOSPACE)
            .size(13),
        actions,
    ]
    .spacing(10);

    mouse_area(
        container(body)
            .padding(14)
            .style(iced::widget::container::rounded_box),
    )
    .on_right_press(Message::OpenCertificateContext(certificate.id))
    .into()
}

fn identity_card<'a>(app: &'a App, identity: &'a Identity) -> Element<'a, Message> {
    let actions: Element<'a, Message> = if app.active_identity_context == Some(identity.id) {
        container(row![button("Edit").on_press(Message::EditIdentity(identity.id))].spacing(8))
            .into()
    } else {
        Space::new().height(Length::Shrink).into()
    };

    let body = column![
        row![
            text(identity.name.clone()).size(18),
            Space::new().width(Length::Fill),
            text(format!("#{}", identity.id))
                .size(13)
                .color(Color::from_rgb8(108, 117, 138)),
        ],
        text(format!("username: {}", empty_as_dash(&identity.username))).size(14),
        text(format!(
            "certificate_id: {}",
            identity
                .certificate_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".into())
        ))
        .size(13)
        .color(Color::from_rgb8(108, 117, 138)),
        actions,
    ]
    .spacing(10);

    mouse_area(
        container(body)
            .padding(14)
            .style(iced::widget::container::rounded_box),
    )
    .on_right_press(Message::OpenIdentityContext(identity.id))
    .into()
}

fn profile_drawer(editor: &ProfileEditor) -> iced::widget::Container<'_, Message> {
    drawer_shell(
        "Edit Profile",
        column![
            labeled_input("Name", &editor.name, |value| {
                Message::ProfileFieldChanged(ProfileField::Name, value)
            }),
            labeled_input(
                "Type (ssh / local)",
                editor.profile_type.as_str(),
                Message::ProfileTypeChanged
            ),
            labeled_input("Group ID", &editor.group_id, |value| {
                Message::ProfileFieldChanged(ProfileField::GroupId, value)
            }),
            labeled_input("Certificate ID", &editor.certificate_id, |value| {
                Message::ProfileFieldChanged(ProfileField::CertificateId, value)
            }),
            labeled_input("Identity ID", &editor.identity_id, |value| {
                Message::ProfileFieldChanged(ProfileField::IdentityId, value)
            }),
            labeled_input("Host", &editor.host, |value| {
                Message::ProfileFieldChanged(ProfileField::Host, value)
            }),
            labeled_input("Port", &editor.port, |value| {
                Message::ProfileFieldChanged(ProfileField::Port, value)
            }),
            labeled_input("Username", &editor.username, |value| {
                Message::ProfileFieldChanged(ProfileField::Username, value)
            }),
            labeled_input("Password", &editor.password, |value| {
                Message::ProfileFieldChanged(ProfileField::Password, value)
            }),
            labeled_input("Theme ID", &editor.theme_id, |value| {
                Message::ProfileFieldChanged(ProfileField::ThemeId, value)
            }),
            labeled_input("Startup Command", &editor.startup_command, |value| {
                Message::ProfileFieldChanged(ProfileField::StartupCommand, value)
            }),
            row![
                button("Close").on_press(Message::CloseDrawer),
                button("Save").on_press(Message::SaveProfile),
            ]
            .spacing(10),
        ]
        .spacing(10),
    )
}

fn certificate_drawer(editor: &CertificateEditor) -> iced::widget::Container<'_, Message> {
    drawer_shell(
        "Edit Certificate",
        column![
            labeled_input("Name", &editor.name, |value| {
                Message::CertificateFieldChanged(CertificateField::Name, value)
            }),
            labeled_area("Private Key", &editor.private_key, |value| {
                Message::CertificateFieldChanged(CertificateField::PrivateKey, value)
            }),
            labeled_area("Public Key", &editor.public_key, |value| {
                Message::CertificateFieldChanged(CertificateField::PublicKey, value)
            }),
            row![
                button("Close").on_press(Message::CloseDrawer),
                button("Save").on_press(Message::SaveCertificate),
            ]
            .spacing(10),
        ]
        .spacing(10),
    )
}

fn identity_drawer(editor: &IdentityEditor) -> iced::widget::Container<'_, Message> {
    drawer_shell(
        "Edit Identity",
        column![
            labeled_input("Name", &editor.name, |value| {
                Message::IdentityFieldChanged(IdentityField::Name, value)
            }),
            labeled_input("Username", &editor.username, |value| {
                Message::IdentityFieldChanged(IdentityField::Username, value)
            }),
            labeled_input("Password", &editor.password, |value| {
                Message::IdentityFieldChanged(IdentityField::Password, value)
            }),
            labeled_input("Certificate ID", &editor.certificate_id, |value| {
                Message::IdentityFieldChanged(IdentityField::CertificateId, value)
            }),
            row![
                button("Close").on_press(Message::CloseDrawer),
                button("Save").on_press(Message::SaveIdentity),
            ]
            .spacing(10),
        ]
        .spacing(10),
    )
}

fn drawer_shell<'a>(
    title: &'a str,
    content: iced::widget::Column<'a, Message>,
) -> iced::widget::Container<'a, Message> {
    container(
        scrollable(column![text(title).size(24), content].spacing(14))
            .height(Length::Fill)
            .width(Length::Fill)
            .direction(iced::widget::scrollable::Direction::Vertical(
                iced::widget::scrollable::Scrollbar::default(),
            )),
    )
    .padding(16)
}

fn menu_buttons(app: &App) -> iced::widget::Column<'_, Message> {
    ManageMenu::ALL
        .into_iter()
        .fold(column![].spacing(8), |column, item| {
            column.push(
                button(text(item.title()))
                    .style(if app.selected_menu == item {
                        iced::widget::button::primary
                    } else {
                        iced::widget::button::secondary
                    })
                    .width(Length::Fill)
                    .on_press(Message::SelectMenu(item)),
            )
        })
}

fn terminal_tab_buttons(app: &App) -> iced::widget::Row<'_, Message> {
    app.terminal_tabs
        .iter()
        .fold(row![].spacing(6), |row, tab| {
            row.push(
                container(
                    row![
                        button(text(tab.title.clone()))
                            .style(if app.active_tab == ActiveTab::Terminal(tab.id) {
                                button::primary
                            } else {
                                button::secondary
                            })
                            .on_press(Message::ActivateTerminal(tab.id)),
                        button("x")
                            .style(iced::widget::button::text)
                            .on_press(Message::CloseTerminal(tab.id)),
                    ]
                    .spacing(4)
                    .align_y(Vertical::Center),
                )
                .style(iced::widget::container::transparent),
            )
        })
}

fn titlebar_controls(app: &App) -> iced::widget::Row<'_, Message> {
    let disconnect = if let Some(tab) = app.current_terminal() {
        button("Disconnect").on_press(Message::TerminalDisconnect(tab.id))
    } else {
        button("Settings").on_press(Message::OpenSettingsWindow)
    };

    row![disconnect].spacing(8)
}

fn mac_titlebar_spacer<'a>() -> iced::widget::Container<'a, Message> {
    container(Space::new().width(Length::Fixed(76.0))).width(Length::Fixed(76.0))
}

fn ansi_color_inputs(colors: &[String; 16]) -> iced::widget::Column<'_, Message> {
    const LABELS: [&str; 16] = [
        "ANSI 0", "ANSI 1", "ANSI 2", "ANSI 3", "ANSI 4", "ANSI 5", "ANSI 6", "ANSI 7", "ANSI 8",
        "ANSI 9", "ANSI 10", "ANSI 11", "ANSI 12", "ANSI 13", "ANSI 14", "ANSI 15",
    ];

    colors
        .iter()
        .enumerate()
        .fold(column![].spacing(8), |column, (index, value)| {
            column.push(labeled_input(LABELS[index], value, move |next| {
                Message::SettingsColorChanged(ColorField::Ansi(index), next)
            }))
        })
}

fn labeled_input<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'static + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label).size(13),
        text_input(label, value).on_input(on_input).padding(10),
    ]
    .spacing(4)
}

fn labeled_pick_list<'a>(
    label: &'a str,
    options: &'a [String],
    selected: Option<&'a String>,
    placeholder: &'a str,
    on_select: impl Fn(String) -> Message + 'a + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label).size(13),
        pick_list(options, selected, on_select)
            .placeholder(placeholder)
            .width(Length::Fill)
            .padding(10),
    ]
    .spacing(4)
}

fn labeled_area<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'static + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label).size(13),
        text_input(label, value)
            .on_input(on_input)
            .padding(10)
            .size(14),
    ]
    .spacing(4)
}

fn settings_section<'a>(
    title: &'a str,
    content: iced::widget::Column<'a, Message>,
) -> iced::widget::Container<'a, Message> {
    container(column![text(title).size(20), content].spacing(10))
        .padding(16)
        .style(iced::widget::container::rounded_box)
}

fn open_settings_window(app: &mut App) -> Task<Message> {
    if app.settings_window.is_some() {
        return Task::none();
    }

    let (_id, task) = window::open(settings_window_settings());
    task.map(Message::SettingsWindowReady)
}

fn panel_style(background: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        border: border::rounded(0.0),
        ..Default::default()
    }
}

fn main_window_settings() -> window::Settings {
    let mut settings = window::Settings {
        size: iced::Size::new(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT),
        min_size: Some(iced::Size::new(980.0, 680.0)),
        transparent: true,
        ..window::Settings::default()
    };

    #[cfg(target_os = "macos")]
    {
        settings.platform_specific.title_hidden = true;
        settings.platform_specific.titlebar_transparent = true;
        settings.platform_specific.fullsize_content_view = true;
    }

    settings
}

fn settings_window_settings() -> window::Settings {
    window::Settings {
        size: iced::Size::new(SETTINGS_WINDOW_WIDTH, SETTINGS_WINDOW_HEIGHT),
        min_size: Some(iced::Size::new(640.0, 520.0)),
        ..window::Settings::default()
    }
}

fn parse_optional_i64(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<i64>().ok()
    }
}

fn empty_as_dash(value: &str) -> String {
    if value.trim().is_empty() {
        "-".into()
    } else {
        value.to_string()
    }
}

fn short_preview(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "-".into()
    } else if trimmed.len() > 64 {
        format!("{}...", &trimmed[..64])
    } else {
        trimmed.to_string()
    }
}

fn normalize_font_family_choice(current: &str, available_fonts: &[String]) -> String {
    canonical_terminal_font_name(current)
        .and_then(|canonical| {
            available_fonts
                .iter()
                .find(|font| font.eq_ignore_ascii_case(&canonical))
                .cloned()
                .or(Some(canonical))
        })
        .unwrap_or_else(|| "monospace".into())
}
