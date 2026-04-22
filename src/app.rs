use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::advanced::input_method;
use iced::alignment::{Horizontal, Vertical};
use iced::event;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::key::{Code, Physical};
use iced::mouse;
use iced::theme::Theme;
use iced::time;
use iced::widget::{
    Space, button, container, mouse_area, opaque, pick_list, row, rule, scrollable, stack, text,
    text_editor, text_input,
};
use iced::window;
use iced::{Background, Border, Color, Element, Length, Point, Shadow, Subscription, Task, Vector};

use crate::models::{
    Connection, ConnectionType, Group, Identity, Key as SshKey, KnownHostEntry, ManageMenu,
    PortForward, PortForwardType, SftpEntry,
};
use crate::persistence::{
    AppPaths, AppSettings, Database, ShortcutSettings, TerminalAnsiGroup, TerminalColors,
    TerminalThemeEntry, builtin_terminal_theme_by_id, builtin_terminal_themes,
    load_custom_terminal_themes, load_settings, read_known_hosts, save_settings,
};
use crate::session::{
    ConnectionTarget, PortForwardHandle, SessionCommand, SessionEvent, SessionHandle, SftpHandle,
    connect_sftp_target, connect_target, sftp_list_dir, sftp_read_file_preview,
    start_port_forward,
};
use crate::terminal::{
    GlyphAtlas, TerminalAtlas, TerminalCanvasEvent, TerminalEvent, TerminalFont, TerminalPoint,
    TerminalSelection, TerminalSnapshot, TerminalTheme, TerminalView, available_terminal_fonts,
    canonical_terminal_font_name,
};

mod constants;
mod helpers;
mod icons;
mod shortcuts;
mod state;
mod styles;
mod update;
mod view;

use update::update;
use view::{style, subscription, theme, title, view};

pub(crate) use constants::*;
pub(crate) use helpers::*;
pub(crate) use icons::*;
pub(crate) use shortcuts::*;
pub(crate) use state::*;
pub(crate) use styles::*;

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
