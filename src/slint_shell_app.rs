#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::models::{
    Connection, ConnectionType, Group, Identity, Key as SshKey, KnownHostEntry, ManageMenu,
    PortForward, Snippet,
};
use crate::persistence::{
    AppPaths, AppSettings, Database, TerminalColors, TerminalSettings, TerminalThemeEntry,
    builtin_terminal_theme_by_id, load_custom_terminal_themes, load_settings,
};
use crate::session::{
    ConnectionTarget, SessionCommand, SessionEvent, SessionHandle, connect_target,
};
use crate::slint_terminal_core::{
    TerminalCell, TerminalColor, TerminalEvent, TerminalFont, TerminalKeyModifiers, TerminalPoint,
    TerminalSelection, TerminalSnapshot, TerminalTheme, TerminalUnderlineStyle, TerminalView,
    normalize_selection, selection_contents,
};
use crate::workspace;
use alacritty_terminal::vte::ansi::CursorShape;
use copypasta::{ClipboardContext, ClipboardProvider};
use slint::ComponentHandle;
use slint::winit_030::winit;
use slint::{Timer, TimerMode};

const SHELL_LOG_LIMIT: usize = 200;
const TERMINAL_COLS: u16 = 96;
const TERMINAL_ROWS: u16 = 32;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(600);
const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);

struct TerminalTab {
    id: String,
    name: String,
    terminal: TerminalView,
    session: SessionHandle,
    theme: TerminalTheme,
    font: TerminalFont,
    line_height: f32,
    pending_session_events: Arc<Mutex<VecDeque<SessionEvent>>>,
    cols: usize,
    rows: usize,
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
    selection_anchor: Option<TerminalPoint>,
    selection: Option<TerminalSelection>,
    dragging_selection: bool,
    terminal_mouse_button_down: bool,
    focused: bool,
    last_click_at: Option<Instant>,
    last_click_point: Option<TerminalPoint>,
    click_count: u8,
    cursor_visible: bool,
    cursor_blink_started_at: Instant,
    last_cursor_blink_key: Option<CursorBlinkKey>,
    window_title: String,
    default_window_title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CursorBlinkKey {
    line: usize,
    column: usize,
    width: usize,
    shape: CursorShape,
    show_cursor: bool,
    blinking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CursorOverlay {
    visible: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: TerminalColor,
}

impl CursorBlinkKey {
    fn from_snapshot(snapshot: &TerminalSnapshot) -> Self {
        Self {
            line: snapshot.cursor_line,
            column: snapshot.cursor_column,
            width: snapshot.cursor_width,
            shape: snapshot.cursor_shape,
            show_cursor: snapshot.show_cursor,
            blinking: snapshot.cursor_blinking,
        }
    }
}

impl TerminalTab {
    fn new(
        id: String,
        name: String,
        terminal: TerminalView,
        session: SessionHandle,
        theme: TerminalTheme,
        font: TerminalFont,
        line_height: f32,
        pending_session_events: Arc<Mutex<VecDeque<SessionEvent>>>,
        default_window_title: String,
    ) -> Self {
        let pixel_width = (TERMINAL_COLS as f32 * font.metrics.cell_width).ceil() as u32;
        let pixel_height = (TERMINAL_ROWS as f32 * font.metrics.cell_height).ceil() as u32;

        Self {
            id,
            name,
            terminal,
            session,
            theme,
            font,
            line_height,
            pending_session_events,
            cols: TERMINAL_COLS as usize,
            rows: TERMINAL_ROWS as usize,
            pixel_width,
            pixel_height,
            scale_factor: 1.0,
            selection_anchor: None,
            selection: None,
            dragging_selection: false,
            terminal_mouse_button_down: false,
            focused: true,
            last_click_at: None,
            last_click_point: None,
            click_count: 0,
            cursor_visible: true,
            cursor_blink_started_at: Instant::now(),
            last_cursor_blink_key: None,
            window_title: default_window_title.clone(),
            default_window_title,
        }
    }

    fn drain_session_events(&mut self) -> bool {
        let mut dirty = false;
        let mut events = Vec::new();
        if let Ok(mut pending) = self.pending_session_events.lock() {
            events.extend(pending.drain(..));
        }
        for event in events {
            match event {
                SessionEvent::Output(bytes) => {
                    self.terminal.feed(&bytes);
                    dirty = true;
                }
                SessionEvent::Error(message) => {
                    self.terminal
                        .push_local_line(&format!("Disconnected: {message}"));
                    dirty = true;
                }
                SessionEvent::Disconnected(reason) => {
                    self.terminal
                        .push_local_line(&format!("Disconnected: {reason}"));
                    dirty = true;
                }
                SessionEvent::Connected { .. } | SessionEvent::Status(_) => {}
            }
        }
        dirty
    }

    fn drain_terminal_events(&mut self) -> bool {
        let mut dirty = false;
        while let Some(event) = self.terminal.try_recv_event() {
            match event {
                TerminalEvent::Title(title) => {
                    self.window_title = if title.is_empty() {
                        self.default_window_title.clone()
                    } else {
                        title
                    };
                    dirty = true;
                }
                TerminalEvent::ResetTitle => {
                    self.window_title = self.default_window_title.clone();
                    dirty = true;
                }
            }
        }
        dirty
    }

    fn update_cursor_blink(&mut self, now: Instant) -> bool {
        let snapshot = self.terminal.snapshot(&self.theme);
        let key = CursorBlinkKey::from_snapshot(&snapshot);
        if self.last_cursor_blink_key != Some(key) {
            self.last_cursor_blink_key = Some(key);
            self.cursor_visible = true;
            self.cursor_blink_started_at = now;
            return true;
        }
        let next_visible = if snapshot.show_cursor && snapshot.cursor_blinking {
            cursor_visible_for_elapsed(now.saturating_duration_since(self.cursor_blink_started_at))
        } else {
            true
        };
        if self.cursor_visible == next_visible {
            return false;
        }
        self.cursor_visible = next_visible;
        true
    }

    fn sync_font_metrics(&mut self, native_cell_width: f32, native_cell_height: f32) -> bool {
        self.font
            .apply_native_metrics(native_cell_width, native_cell_height, self.line_height)
    }

    fn sync_window_size(&mut self, pixel_width: u32, pixel_height: u32, scale_factor: f32) -> bool {
        let Some(grid) = terminal_grid_size(pixel_width, pixel_height, scale_factor, &self.font)
        else {
            return false;
        };
        let dimensions_changed = grid.0 != self.cols || grid.1 != self.rows;
        let pixels_changed = pixel_width != self.pixel_width
            || pixel_height != self.pixel_height
            || (scale_factor - self.scale_factor).abs() > f32::EPSILON;
        if !dimensions_changed && !pixels_changed {
            return false;
        }
        self.cols = grid.0;
        self.rows = grid.1;
        self.pixel_width = pixel_width;
        self.pixel_height = pixel_height;
        self.scale_factor = scale_factor.max(1.0);
        if dimensions_changed {
            self.terminal.resize(grid.0, grid.1);
            let _ = self.session.command_tx.send(SessionCommand::Resize {
                cols: grid.0.min(u16::MAX as usize) as u16,
                rows: grid.1.min(u16::MAX as usize) as u16,
            });
        }
        true
    }

    fn scroll(&mut self, delta_y: f32, x: f32, y: f32) -> bool {
        let Some(point) = self.terminal.point_for_logical_position(
            x,
            y,
            self.font.metrics.cell_width,
            self.font.metrics.cell_height,
        ) else {
            return false;
        };
        let lines = delta_y / self.font.metrics.cell_height.max(1.0);
        let lines = if lines.abs() < 1.0 {
            lines.signum() as i32
        } else {
            lines.round() as i32
        };
        if lines == 0 {
            return false;
        }
        self.terminal.handle_scroll(lines, point);
        true
    }

    fn pointer_down(&mut self, x: f32, y: f32) -> bool {
        let Some(point) = self.terminal.point_for_logical_position(
            x,
            y,
            self.font.metrics.cell_width,
            self.font.metrics.cell_height,
        ) else {
            self.dragging_selection = false;
            return false;
        };
        if self.terminal.handle_mouse_press(point) {
            self.terminal_mouse_button_down = true;
            self.dragging_selection = false;
            self.selection_anchor = None;
            let had_selection = self.selection.take().is_some();
            return had_selection;
        }
        let now = Instant::now();
        let is_multi_click = self.last_click_at.map_or(false, |t| {
            now.duration_since(t) < MULTI_CLICK_INTERVAL && self.last_click_point == Some(point)
        });
        self.click_count = if is_multi_click {
            (self.click_count + 1).min(3)
        } else {
            1
        };
        self.last_click_at = Some(now);
        self.last_click_point = Some(point);
        self.dragging_selection = true;
        self.selection_anchor = Some(point);
        self.selection = match self.click_count {
            2 => {
                let sel = self.terminal.word_selection_at_point(&self.theme, point);
                Some(TerminalSelection {
                    start: sel.start,
                    end: sel.end,
                })
            }
            3 => {
                let sel = self.terminal.token_selection_at_point(&self.theme, point);
                Some(TerminalSelection {
                    start: sel.start,
                    end: sel.end,
                })
            }
            _ => Some(TerminalSelection {
                start: point,
                end: point,
            }),
        };
        self.selection.is_some()
    }

    fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        if self.terminal_mouse_button_down {
            let Some(point) = self.terminal.point_for_logical_position(
                x,
                y,
                self.font.metrics.cell_width,
                self.font.metrics.cell_height,
            ) else {
                return false;
            };
            return self.terminal.handle_mouse_drag(point);
        }
        if !self.dragging_selection {
            return false;
        }
        let point = self.terminal.clamped_point_for_logical_position(
            x,
            y,
            self.font.metrics.cell_width,
            self.font.metrics.cell_height,
        );
        if self.click_count >= 2 {
            let anchor = self.selection_anchor.unwrap_or(point);
            let (new_start, new_end) = if self.click_count >= 3 {
                let sel = self.terminal.token_selection_at_point(&self.theme, point);
                (sel.start, sel.end)
            } else {
                let sel = self.terminal.word_selection_at_point(&self.theme, point);
                (sel.start, sel.end)
            };
            let norm = normalize_selection(anchor, point);
            let merged_start =
                if (new_start.line, new_start.column) < (norm.start.line, norm.start.column) {
                    new_start
                } else {
                    norm.start
                };
            let merged_end = if (new_end.line, new_end.column) > (norm.end.line, norm.end.column) {
                new_end
            } else {
                norm.end
            };
            self.selection = Some(TerminalSelection {
                start: merged_start,
                end: merged_end,
            });
        } else {
            self.selection = Some(TerminalSelection {
                start: self.selection_anchor.unwrap_or(point),
                end: point,
            });
        }
        true
    }

    fn pointer_up(&mut self, x: f32, y: f32) {
        if self.terminal_mouse_button_down {
            if let Some(point) = self.terminal.point_for_logical_position(
                x,
                y,
                self.font.metrics.cell_width,
                self.font.metrics.cell_height,
            ) {
                self.terminal.handle_mouse_release(point);
            }
            self.terminal_mouse_button_down = false;
            return;
        }
        self.dragging_selection = false;
    }

    fn focus_changed(&mut self, focused: bool) -> bool {
        if self.focused == focused {
            return false;
        }
        self.focused = focused;
        self.terminal.handle_focus_change(focused);
        if focused {
            self.cursor_visible = true;
            self.cursor_blink_started_at = Instant::now();
        }
        !focused && self.selection.is_some()
    }

    fn selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        let snapshot = self.terminal.snapshot(&self.theme);
        selection_contents(&snapshot, Some(sel))
    }

    fn paste_text(&mut self, text: &str) -> bool {
        let encoded = self.terminal.encode_text_input(text);
        self.send_terminal_input(encoded)
    }

    fn send_terminal_input(&mut self, bytes: Vec<u8>) -> bool {
        self.session
            .command_tx
            .send(SessionCommand::Input(bytes))
            .is_ok()
    }

    fn disconnect(&self, reason: &str) {
        let _ = self
            .session
            .command_tx
            .send(SessionCommand::Disconnect(reason.to_string()));
    }
}

fn cursor_visible_for_elapsed(elapsed: Duration) -> bool {
    (elapsed.as_millis() / CURSOR_BLINK_INTERVAL.as_millis()) % 2 == 0
}

fn terminal_grid_size(
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
    font: &TerminalFont,
) -> Option<(usize, usize)> {
    let cell_width = font.metrics.cell_width;
    let cell_height = font.metrics.cell_height;
    if cell_width <= 0.0 || cell_height <= 0.0 {
        return None;
    }
    let logical_width = pixel_width as f32 / scale_factor.max(1.0);
    let logical_height = pixel_height as f32 / scale_factor.max(1.0);
    let cols = (logical_width / cell_width).floor().max(1.0) as usize;
    let rows = (logical_height / cell_height).floor().max(1.0) as usize;
    Some((cols, rows))
}

slint::slint! {
    import { LineEdit } from "std-widgets.slint";

    export struct ShellNavItem {
        index: int,
        title: string,
        active: bool,
    }

    export struct ShellStatItem {
        title: string,
        value: string,
        caption: string,
    }

    export struct ShellConnectionItem {
        id: string,
        name: string,
        endpoint: string,
        badge: string,
        initial: string,
        accent: color,
    }

    export struct ShellListItem {
        id: string,
        title: string,
        subtitle: string,
        badge: string,
        initial: string,
        accent: color,
    }

    export struct TerminalCellItem {
        text: string,
        x: length,
        y: length,
        width: length,
        height: length,
        foreground: color,
        background: color,
        bold: bool,
        italic: bool,
    }

    export struct TerminalDecorationItem {
        x: length,
        y: length,
        width: length,
        height: length,
        color: color,
    }

    export struct TabItem {
        id: string,
        title: string,
        active: bool,
        is-terminal: bool,
    }

    component NavRow inherits Rectangle {
        in property <ShellNavItem> item;
        callback clicked();
        height: 36px;
        border-radius: 9px;
        background: item.active ? #e7edf0 : transparent;

        Rectangle {
            x: 14px;
            y: 10px;
            width: 16px;
            height: 16px;
            border-radius: 4px;
            border-width: 1px;
            border-color: item.active ? #111827 : #7b8794;
            background: item.active ? #111827 : transparent;
        }

        Text {
            x: 44px;
            y: 0px;
            width: parent.width - 54px;
            height: 36px;
            text: item.title;
            color: item.active ? #111827 : #435064;
            font-size: 14px;
            font-weight: item.active ? 700 : 500;
            vertical-alignment: center;
        }

        TouchArea {
            width: 100%;
            height: 100%;
            clicked => {
                root.clicked();
            }
        }
    }

    component StatCard inherits Rectangle {
        in property <ShellStatItem> stat;
        width: 156px;
        height: 92px;
        border-radius: 16px;
        background: #ffffff;
        border-width: 1px;
        border-color: #dde5e8;

        Text {
            x: 18px;
            y: 16px;
            width: parent.width - 36px;
            height: 18px;
            text: stat.title;
            color: #657386;
            font-size: 12px;
            font-weight: 600;
        }

        Text {
            x: 18px;
            y: 36px;
            width: parent.width - 36px;
            height: 28px;
            text: stat.value;
            color: #111827;
            font-size: 24px;
            font-weight: 700;
        }

        Text {
            x: 18px;
            y: 66px;
            width: parent.width - 36px;
            height: 18px;
            text: stat.caption;
            color: #7b8794;
            font-size: 12px;
        }
    }

    component ConnectionRow inherits Rectangle {
        in property <ShellConnectionItem> connection;
        in property <bool> selected;
        callback clicked();
        callback action(string);
        width: 360px;
        height: 60px;
        border-radius: 16px;
        background: selected ? #ffffff : #fbfcfc;
        border-width: selected ? 2px : 1px;
        border-color: selected ? #1494ff : #dfe7ea;

        Rectangle {
            x: 18px;
            y: 14px;
            width: 32px;
            height: 32px;
            border-radius: 9px;
            background: connection.accent;

            Text {
                width: 100%;
                height: 100%;
                text: connection.initial;
                color: #ffffff;
                font-size: 14px;
                font-weight: 700;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Text {
            x: 66px;
            y: 12px;
            width: parent.width - 190px;
            height: 20px;
            text: connection.name;
            color: #111827;
            font-size: 14px;
            font-weight: 700;
            overflow: elide;
        }

        Text {
            x: 66px;
            y: 32px;
            width: parent.width - 190px;
            height: 18px;
            text: connection.endpoint;
            color: #66758a;
            font-size: 12px;
            overflow: elide;
        }

        Rectangle {
            x: parent.width - 126px;
            y: 20px;
            width: 46px;
            height: 20px;
            border-radius: 10px;
            background: #eef4f6;

            Text {
                width: 100%;
                height: 100%;
                text: connection.badge;
                color: #526174;
                font-size: 10px;
                font-weight: 700;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        TouchArea {
            width: 100%;
            height: 100%;
            clicked => {
                root.clicked();
            }
        }
    }

    component ListRow inherits Rectangle {
        in property <ShellListItem> item;
        width: 360px;
        height: 60px;
        border-radius: 16px;
        background: #fbfcfc;
        border-width: 1px;
        border-color: #dfe7ea;

        Rectangle {
            x: 18px;
            y: 14px;
            width: 32px;
            height: 32px;
            border-radius: 9px;
            background: item.accent;

            Text {
                width: 100%;
                height: 100%;
                text: item.initial;
                color: #ffffff;
                font-size: 14px;
                font-weight: 700;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Text {
            x: 66px;
            y: 12px;
            width: parent.width - 150px;
            height: 20px;
            text: item.title;
            color: #111827;
            font-size: 14px;
            font-weight: 700;
            overflow: elide;
        }

        Text {
            x: 66px;
            y: 32px;
            width: parent.width - 150px;
            height: 18px;
            text: item.subtitle;
            color: #66758a;
            font-size: 12px;
            overflow: elide;
        }

        Rectangle {
            x: parent.width - 74px;
            y: 20px;
            width: 56px;
            height: 20px;
            border-radius: 10px;
            background: #eef4f6;

            Text {
                width: 100%;
                height: 100%;
                text: item.badge;
                color: #526174;
                font-size: 10px;
                font-weight: 700;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }
    }

    export component TimonSlintShellWindow inherits Window {
        in property <[ShellNavItem]> nav-items;
        in property <[ShellStatItem]> stats;
        in property <[ShellListItem]> group-items;
        in property <[ShellConnectionItem]> connections;
        in property <[ShellListItem]> list-items;
        in property <int> active-menu-index;
        in property <int> connection-group-rows;
        in property <string> page-title;
        in property <string> page-subtitle;
        in property <string> connection-count-label;
        in property <string> group-count-label;
        in property <string> selected-connection-id;
        in property <string> selected-connection-name;
        in property <string> selected-connection-endpoint;
        in property <string> selected-connection-type;
        in property <string> connect-status;
        in property <string> search-query;

        // Tab system
        in property <[TabItem]> tabs;
        in property <int> active-tab-index: -1;

        // Terminal rendering
        in property <[TerminalCellItem]> terminal-cells;
        in property <[TerminalDecorationItem]> terminal-decorations;
        in property <color> terminal-background: #0f1419;
        in property <string> terminal-font-family: "monospace";
        in property <length> terminal-font-size: 13px;
        out property <length> terminal-native-cell-width: terminal-font-measure.preferred-width / 10;
        out property <length> terminal-native-cell-height: terminal-font-measure.font-metrics.ascent - terminal-font-measure.font-metrics.descent;
        in property <bool> cursor-overlay-visible: false;
        in property <length> cursor-overlay-x: 0px;
        in property <length> cursor-overlay-y: 0px;
        in property <length> cursor-overlay-width: 0px;
        in property <length> cursor-overlay-height: 0px;
        in property <color> cursor-overlay-color: #ffffff;

        callback select-menu(int);
        callback select-connection(string);
        callback connect-selected-connection();
        callback open-connection(string);
        callback search-changed(string);
        callback select-tab(int);
        callback close-tab(int);
        callback terminal-input(string, bool, bool, bool, bool);
        callback terminal-pointer-down(float, float);
        callback terminal-pointer-moved(float, float);
        callback terminal-pointer-up(float, float);
        callback terminal-scroll(float, float, float);
        callback terminal-focus-changed(bool);
        callback session-event-ready();

        title: active-tab-index >= 0 && active-tab-index < tabs.length ? tabs[active-tab-index].title : "Timon";
        background: #edf1f2;

        // Font measurement (always present, invisible)
        terminal-font-measure := Text {
            visible: false;
            text: "MMMMMMMMMM";
            font-family: root.terminal-font-family;
            font-size: root.terminal-font-size;
            font-weight: 400;
        }

        Rectangle {
            width: 100%;
            height: 52px;
            background: #3f465c;

            Rectangle { x: 14px; y: 20px; width: 12px; height: 12px; border-radius: 6px; background: #8a91a5; }
            Rectangle { x: 34px; y: 20px; width: 12px; height: 12px; border-radius: 6px; background: #8a91a5; }
            Rectangle { x: 54px; y: 20px; width: 12px; height: 12px; border-radius: 6px; background: #8a91a5; }

            Rectangle {
                x: 84px;
                y: 10px;
                width: 132px;
                height: 32px;
                border-radius: 10px;
                background: #535a70;

                Text {
                    x: 16px;
                width: parent.width - 32px;
                height: 100%;
                    text: root.page-title;
                    color: #ffffff;
                    font-size: 13px;
                    font-weight: 700;
                    vertical-alignment: center;
                }
            }
        }

        Rectangle {
            x: 0px;
            y: 52px;
            width: 184px;
            height: parent.height - 52px;
            background: #f7f9fa;
            border-width: 0px;

            Text {
                x: 18px;
                y: 24px;
                width: parent.width - 36px;
                height: 28px;
                text: "Timon";
                color: #111827;
                font-size: 18px;
                font-weight: 700;
            }

            for item[index] in root.nav-items: NavRow {
                x: 10px;
                y: 74px + index * 44px;
                width: parent.width - 20px;
                item: item;
                clicked => {
                    root.select-menu(item.index);
                }
            }

            Rectangle {
                x: 0px;
                y: parent.height - 78px;
                width: parent.width;
                height: 1px;
                background: #dde5e8;
            }

            Text {
                x: 20px;
                y: parent.height - 56px;
                width: parent.width - 40px;
                height: 18px;
                text: "Workspace";
                color: #111827;
                font-size: 12px;
                font-weight: 700;
            }

            Text {
                x: 20px;
                y: parent.height - 36px;
                width: parent.width - 40px;
                height: 16px;
                text: root.connection-count-label + " / " + root.group-count-label;
                color: #657386;
                font-size: 11px;
            }
        }

        // Tab bar
        Rectangle {
            x: 184px;
            y: 52px;
            width: parent.width - 184px;
            height: root.tabs.length > 0 ? 36px : 0px;
            background: #3f465c;

            for tab[index] in root.tabs: Rectangle {
                x: 8px + index * 140px;
                y: 4px;
                width: 130px;
                height: 28px;
                border-radius: 6px;
                background: tab.active ? #535a70 : transparent;

                Text {
                    x: 8px;
                    width: parent.width - 28px;
                    height: 100%;
                    text: tab.title;
                    color: #ffffff;
                    font-size: 12px;
                    vertical-alignment: center;
                    overflow: elide;
                }

                // Close button
                Rectangle {
                    x: parent.width - 22px;
                    y: 6px;
                    width: 16px;
                    height: 16px;
                    border-radius: 8px;
                    background: #8a91a5;

                    Text {
                        width: 100%;
                        height: 100%;
                        text: "×";
                        color: #ffffff;
                        font-size: 12px;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }

                    TouchArea {
                        width: 100%;
                        height: 100%;
                        clicked => {
                            root.close-tab(index);
                        }
                    }
                }

                TouchArea {
                    width: parent.width;
                    height: parent.height;
                    clicked => {
                        root.select-tab(index);
                    }
                }
            }
        }

        // Content area
        Rectangle {
            x: 184px;
            y: 52px + (root.tabs.length > 0 ? 36px : 0px);
            width: parent.width - 184px;
            height: parent.height - 52px - (root.tabs.length > 0 ? 36px : 0px);
            background: root.active-tab-index >= 0 ? root.terminal-background : #edf1f2;

            // Management content (visible when no terminal tab is active)
            Rectangle {
                width: 100%;
                height: 100%;
                visible: root.active-tab-index < 0;

                Rectangle {
                    x: 0px;
                    y: 0px;
                    width: parent.width;
                    height: 56px;
                    background: #e4eaec;

                    LineEdit {
                        x: 18px;
                        y: 10px;
                        width: root.active-menu-index == 0 ? parent.width - 150px : parent.width - 36px;
                        height: 36px;
                        text: root.search-query;
                        placeholder-text: "Find a host or ssh user@hostname...";
                        edited(value) => {
                            root.search-changed(value);
                        }
                    }

                    Rectangle {
                        x: parent.width - 118px;
                        y: 12px;
                        width: 98px;
                        height: 32px;
                        border-radius: 10px;
                        background: #3f465c;
                        visible: root.active-menu-index == 0;

                        Text {
                            width: 100%;
                            height: 100%;
                            text: "CONNECT";
                            color: #ffffff;
                            font-size: 11px;
                            font-weight: 700;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }

                        TouchArea {
                            width: 100%;
                            height: 100%;
                            clicked => {
                                root.connect-selected-connection();
                            }
                        }
                    }
                }

                Text {
                    x: 30px;
                    y: 82px;
                    width: parent.width - 60px;
                    height: 24px;
                    text: root.page-title;
                    color: #111827;
                    font-size: 18px;
                    font-weight: 700;
                }

                Text {
                    x: 30px;
                    y: 106px;
                    width: parent.width - 320px;
                    height: 18px;
                    text: root.page-subtitle;
                    color: #657386;
                    font-size: 12px;
                }

                Text {
                    x: parent.width - 300px;
                    y: 106px;
                    width: 270px;
                    height: 18px;
                    text: root.connect-status;
                    color: #657386;
                    font-size: 12px;
                    horizontal-alignment: right;
                    overflow: elide;
                }

                for stat[index] in root.stats: StatCard {
                    x: 30px + index * 172px;
                    y: 136px;
                    stat: stat;
                }

                Rectangle {
                    x: parent.width - 236px;
                    y: 100px;
                    width: 206px;
                    height: 128px;
                    border-radius: 16px;
                    background: #ffffff;
                    border-width: 1px;
                    border-color: #dce5e8;
                    visible: root.active-menu-index == 0;

                    Text {
                        x: 18px;
                        y: 16px;
                        width: parent.width - 36px;
                        height: 18px;
                        text: "Selected";
                        color: #657386;
                        font-size: 12px;
                        font-weight: 700;
                    }

                    Text {
                        x: 18px;
                        y: 40px;
                        width: parent.width - 36px;
                        height: 22px;
                        text: root.selected-connection-name;
                        color: #111827;
                        font-size: 15px;
                        font-weight: 700;
                        overflow: elide;
                    }

                    Text {
                        x: 18px;
                        y: 66px;
                        width: parent.width - 36px;
                        height: 18px;
                        text: root.selected-connection-endpoint;
                        color: #657386;
                        font-size: 12px;
                        overflow: elide;
                    }

                    Rectangle {
                        x: 18px;
                        y: 94px;
                        width: 70px;
                        height: 20px;
                        border-radius: 10px;
                        background: #eef4f6;

                        Text {
                            width: 100%;
                            height: 100%;
                            text: root.selected-connection-type;
                            color: #526174;
                            font-size: 10px;
                            font-weight: 700;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                    }
                }

                Text {
                    x: 30px;
                    y: 256px;
                    width: parent.width - 60px;
                    height: 22px;
                    text: "Groups";
                    color: #111827;
                    font-size: 15px;
                    font-weight: 700;
                    visible: root.active-menu-index == 0;
                }

                for item[index] in root.group-items: ListRow {
                    x: 30px + Math.mod(index, 2) * 378px;
                    y: 292px + Math.floor(index / 2) * 74px;
                    item: item;
                    visible: root.active-menu-index == 0;
                }

                Text {
                    x: 30px;
                    y: root.active-menu-index == 0 ? 292px + root.connection-group-rows * 74px + 24px : 256px;
                    width: parent.width - 60px;
                    height: 22px;
                    text: root.page-title;
                    color: #111827;
                    font-size: 15px;
                    font-weight: 700;
                    visible: root.active-menu-index != 0;
                }

                Text {
                    x: 30px;
                    y: 292px + root.connection-group-rows * 74px + 24px;
                    width: parent.width - 60px;
                    height: 22px;
                    text: "Connections";
                    color: #111827;
                    font-size: 15px;
                    font-weight: 700;
                    visible: root.active-menu-index == 0;
                }

                for connection[index] in root.connections: ConnectionRow {
                    x: 30px + Math.mod(index, 2) * 378px;
                    y: 292px + root.connection-group-rows * 74px + 60px + Math.floor(index / 2) * 74px;
                    connection: connection;
                    selected: connection.id == root.selected-connection-id;
                    visible: root.active-menu-index == 0;
                    clicked => {
                        root.select-connection(connection.id);
                        root.open-connection(connection.id);
                    }
                }

                for item[index] in root.list-items: ListRow {
                    x: 30px + Math.mod(index, 2) * 378px;
                    y: 292px + Math.floor(index / 2) * 74px;
                    item: item;
                    visible: root.active-menu-index != 0;
                }
            }

            // Terminal rendering (visible when a terminal tab is active)
            if root.active-tab-index >= 0: FocusScope {
                width: 100%;
                height: 100%;
                focus-on-click: true;

                capture-key-pressed(event) => {
                    if (event.text != "") {
                        root.terminal-input(
                            event.text,
                            event.modifiers.alt,
                            event.modifiers.control,
                            event.modifiers.shift,
                            event.modifiers.meta,
                        );
                        return accept;
                    }
                    return reject;
                }

                focus-gained(_) => {
                    root.terminal-focus-changed(true);
                }

                focus-lost(_) => {
                    root.terminal-focus-changed(false);
                }

                Rectangle {
                    width: 100%;
                    height: 100%;
                    background: root.terminal-background;

                    for cell in root.terminal-cells: Rectangle {
                        x: cell.x;
                        y: cell.y;
                        width: cell.width;
                        height: cell.height;
                        background: cell.background;

                        Text {
                            width: parent.width;
                            height: parent.height;
                            text: cell.text;
                            color: cell.foreground;
                            font-family: root.terminal-font-family;
                            font-size: root.terminal-font-size;
                            font-weight: cell.bold ? 700 : 400;
                            font-italic: cell.italic;
                            vertical-alignment: center;
                        }
                    }

                    for decoration in root.terminal-decorations: Rectangle {
                        x: decoration.x;
                        y: decoration.y;
                        width: decoration.width;
                        height: decoration.height;
                        background: decoration.color;
                    }

                    Rectangle {
                        visible: root.cursor-overlay-visible;
                        x: root.cursor-overlay-x;
                        y: root.cursor-overlay-y;
                        width: root.cursor-overlay-width;
                        height: root.cursor-overlay-height;
                        background: root.cursor-overlay-color;
                    }
                }

                TouchArea {
                    width: 100%;
                    height: 100%;

                    pointer-event(event) => {
                        if (event.button != PointerEventButton.left) {
                            return;
                        }
                        if (event.kind == PointerEventKind.down) {
                            root.terminal-pointer-down(self.mouse-x / 1px, self.mouse-y / 1px);
                        } else if (event.kind == PointerEventKind.move) {
                            root.terminal-pointer-moved(self.mouse-x / 1px, self.mouse-y / 1px);
                        } else if (event.kind == PointerEventKind.up || event.kind == PointerEventKind.cancel) {
                            root.terminal-pointer-up(self.mouse-x / 1px, self.mouse-y / 1px);
                        }
                    }

                    scroll-event(event) => {
                        root.terminal-scroll(event.delta-y / 1px, self.mouse-x / 1px, self.mouse-y / 1px);
                        return accept;
                    }
                }
            }
        }
    }
}

// --- Terminal rendering helpers ---

fn slint_color([r, g, b, a]: [u8; 4]) -> slint::Color {
    slint::Color::from_argb_u8(a, r, g, b)
}

fn slint_selection_contains(selection: Option<&TerminalSelection>, cell: &TerminalCell) -> bool {
    let Some(sel) = selection else {
        return false;
    };
    let norm = normalize_selection(sel.start, sel.end);
    let p = (cell.line, cell.column);
    p >= (norm.start.line, norm.start.column) && p <= (norm.end.line, norm.end.column)
}

fn slint_cursor_covers_cell(snapshot: &TerminalSnapshot, cell: &TerminalCell) -> bool {
    if !snapshot.show_cursor {
        return false;
    }
    let cursor_col = snapshot.cursor_column;
    let cursor_line = snapshot.cursor_line;
    if cell.line != cursor_line {
        return false;
    }
    cell.column >= cursor_col && cell.column < cursor_col + snapshot.cursor_width
}

fn cursor_overlay_from_snapshot(
    snapshot: &TerminalSnapshot,
    font: &TerminalFont,
    cursor_visible: bool,
) -> CursorOverlay {
    if !cursor_visible || !snapshot.show_cursor {
        return CursorOverlay {
            visible: false,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            color: snapshot.cursor_color,
        };
    }
    match snapshot.cursor_shape {
        CursorShape::Block | CursorShape::Hidden => CursorOverlay {
            visible: false,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            color: snapshot.cursor_color,
        },
        CursorShape::Beam => {
            let beam_width = font.metrics.cell_width.max(2.0);
            CursorOverlay {
                visible: true,
                x: snapshot.cursor_column as f32 * font.metrics.cell_width,
                y: snapshot.cursor_line as f32 * font.metrics.cell_height,
                width: beam_width,
                height: font.metrics.cell_height,
                color: snapshot.cursor_color,
            }
        }
        CursorShape::Underline | CursorShape::HollowBlock => {
            let stroke = (font.metrics.cell_height * 0.08).ceil().max(1.0);
            CursorOverlay {
                visible: true,
                x: snapshot.cursor_column as f32 * font.metrics.cell_width,
                y: (snapshot.cursor_line + 1) as f32 * font.metrics.cell_height - stroke,
                width: snapshot.cursor_width as f32 * font.metrics.cell_width,
                height: stroke,
                color: snapshot.cursor_color,
            }
        }
    }
}

fn snapshot_to_shell_cells(
    snapshot: &TerminalSnapshot,
    selection: Option<&TerminalSelection>,
    font: &TerminalFont,
    cursor_visible: bool,
) -> slint::ModelRc<TerminalCellItem> {
    let cells = snapshot
        .cells
        .iter()
        .map(|cell| {
            let selected = slint_selection_contains(selection, cell);
            let cursor_on_cell = slint_cursor_covers_cell(snapshot, cell);
            let foreground = if cursor_on_cell
                && cursor_visible
                && snapshot.show_cursor
                && matches!(snapshot.cursor_shape, CursorShape::Block)
            {
                snapshot.cursor_text
            } else if selected {
                snapshot.selection_foreground
            } else {
                cell.fg
            };
            let background = if cursor_on_cell
                && cursor_visible
                && snapshot.show_cursor
                && matches!(snapshot.cursor_shape, CursorShape::Block)
            {
                snapshot.cursor_color
            } else if selected {
                snapshot.selection_background
            } else {
                cell.bg
            };

            TerminalCellItem {
                text: if cell.hidden {
                    "".into()
                } else {
                    cell.text.clone().into()
                },
                x: cell.column as f32 * font.metrics.cell_width,
                y: cell.line as f32 * font.metrics.cell_height,
                width: cell.width.max(1) as f32 * font.metrics.cell_width,
                height: font.metrics.cell_height,
                foreground: slint_color(foreground.rgba8()),
                background: slint_color(background.rgba8()),
                bold: cell.bold,
                italic: cell.italic,
            }
        })
        .collect::<Vec<_>>();
    slint::ModelRc::new(slint::VecModel::from(cells))
}

fn decoration_stroke_width(font: &TerminalFont) -> f32 {
    (font.metrics.cell_height * 0.06).ceil().max(1.0)
}

fn snapshot_to_shell_decorations(
    snapshot: &TerminalSnapshot,
    font: &TerminalFont,
) -> slint::ModelRc<TerminalDecorationItem> {
    let mut decorations = Vec::new();
    for cell in &snapshot.cells {
        if cell.hidden {
            continue;
        }
        let x = cell.column as f32 * font.metrics.cell_width;
        let y = cell.line as f32 * font.metrics.cell_height;
        let width = cell.width.max(1) as f32 * font.metrics.cell_width;
        let height = font.metrics.cell_height;
        if let Some(style) = cell.underline {
            push_underline(
                &mut decorations,
                x,
                y,
                width,
                height,
                style,
                cell.underline_color,
                font,
            );
        }
        if cell.strikeout {
            push_decoration_rect(
                &mut decorations,
                x,
                y + height * 0.56,
                width,
                decoration_stroke_width(font),
                cell.fg,
            );
        }
    }
    slint::ModelRc::new(slint::VecModel::from(decorations))
}

fn push_underline(
    decorations: &mut Vec<TerminalDecorationItem>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: TerminalUnderlineStyle,
    color: TerminalColor,
    font: &TerminalFont,
) {
    let stroke = decoration_stroke_width(font);
    let baseline_y = y + height - stroke;
    match style {
        TerminalUnderlineStyle::Single => {
            push_decoration_rect(decorations, x, baseline_y, width, stroke, color);
        }
        TerminalUnderlineStyle::Double => {
            push_decoration_rect(decorations, x, baseline_y, width, stroke, color);
            push_decoration_rect(
                decorations,
                x,
                baseline_y - stroke * 2.0,
                width,
                stroke,
                color,
            );
        }
        TerminalUnderlineStyle::Dotted => {
            let gap = (stroke * 2.0).max(2.0);
            let mut cx = x;
            while cx < x + width {
                let seg = gap.min(x + width - cx);
                push_decoration_rect(decorations, cx, baseline_y, seg, stroke, color);
                cx += gap * 2.0;
            }
        }
        TerminalUnderlineStyle::Dashed => {
            let seg = (width * 0.12).max(4.0);
            let gap = (seg * 0.75).max(2.0);
            let mut cx = x;
            while cx < x + width {
                let s = seg.min(x + width - cx);
                push_decoration_rect(decorations, cx, baseline_y, s, stroke, color);
                cx += seg + gap;
            }
        }
        TerminalUnderlineStyle::Curly => {
            let amplitude = (height * 0.12).max(2.0);
            let half_period = (font.metrics.cell_width * 0.5).max(3.0);
            let mut cx = x;
            let mut high = true;
            while cx < x + width {
                let seg = half_period.min(x + width - cx);
                let cy = if high {
                    baseline_y - amplitude
                } else {
                    baseline_y
                };
                push_decoration_rect(decorations, cx, cy, seg, stroke, color);
                cx += seg;
                high = !high;
            }
        }
    }
}

fn push_decoration_rect(
    decorations: &mut Vec<TerminalDecorationItem>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: TerminalColor,
) {
    decorations.push(TerminalDecorationItem {
        x,
        y,
        width,
        height,
        color: slint_color(color.rgba8()),
    });
}

fn is_copy_shortcut(text: &str, modifiers: TerminalKeyModifiers) -> bool {
    (modifiers.control || modifiers.meta)
        && (text == "c" || text == "C")
        && !modifiers.alt
        && !modifiers.shift
}

fn is_paste_shortcut(text: &str, modifiers: TerminalKeyModifiers) -> bool {
    (modifiers.control || modifiers.meta)
        && (text == "v" || text == "V")
        && !modifiers.alt
        && !modifiers.shift
}

fn write_clipboard(text: String) {
    if let Ok(mut ctx) = ClipboardContext::new() {
        let _ = ctx.set_contents(text);
    }
}

fn read_clipboard() -> String {
    ClipboardContext::new()
        .ok()
        .and_then(|mut ctx| ctx.get_contents().ok())
        .unwrap_or_default()
}

fn configure_window_backend() -> anyhow::Result<()> {
    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(|attrs| {
            attrs
                .with_transparent(false)
                .with_inner_size(winit::dpi::LogicalSize::new(920.0, 620.0))
                .with_min_inner_size(winit::dpi::LogicalSize::new(400.0, 300.0))
        })
        .select()
        .map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn run() -> anyhow::Result<()> {
    configure_window_backend()?;
    let paths = Rc::new(AppPaths::discover()?);
    let database = Rc::new(Database::new(&paths.database)?);
    let settings = Rc::new(load_settings(&paths.settings).unwrap_or_default());
    let workspace = Rc::new(RefCell::new(workspace::WorkspaceData::load(
        paths.as_ref(),
        database.as_ref(),
    )));
    let shell_logs = Rc::new(RefCell::new(initial_shell_logs(
        &workspace.borrow(),
        settings.as_ref(),
    )));

    let runtime = Rc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?,
    );
    let terminal_tabs: Rc<RefCell<Vec<TerminalTab>>> = Rc::new(RefCell::new(Vec::new()));
    let active_tab_index: Rc<RefCell<i32>> = Rc::new(RefCell::new(-1));

    let ui = TimonSlintShellWindow::new()?;
    apply_workspace(
        &ui,
        &workspace.borrow(),
        settings.as_ref(),
        shell_logs.borrow().as_slice(),
        ManageMenu::Connections,
        "",
    );
    {
        let workspace = workspace.borrow();
        apply_selected_connection(
            &ui,
            &workspace.connections,
            &initial_selected_connection_id(&workspace.connections),
        );
    }

    // select-menu
    let ui_weak = ui.as_weak();
    let menu_workspace = Rc::clone(&workspace);
    let menu_settings = Rc::clone(&settings);
    let menu_logs = Rc::clone(&shell_logs);
    let menu_tabs = Rc::clone(&terminal_tabs);
    let menu_active = Rc::clone(&active_tab_index);
    ui.on_select_menu(move |menu_index| {
        if let Some(ui) = ui_weak.upgrade() {
            // Switch away from terminal tab to management
            *menu_active.borrow_mut() = -1;
            ui.set_active_tab_index(-1);
            apply_tabs_to_ui(&ui, &menu_tabs.borrow(), -1);

            let active_menu = manage_menu_from_index(menu_index);
            let query = ui.get_search_query().to_string();
            record_shell_log(&menu_logs, format!("Opened {}", active_menu.title()));
            let workspace = menu_workspace.borrow();
            apply_workspace(
                &ui,
                &workspace,
                menu_settings.as_ref(),
                menu_logs.borrow().as_slice(),
                active_menu,
                &query,
            );

            if active_menu == ManageMenu::Connections {
                let selected_id = filtered_selected_connection_id(
                    &workspace.groups,
                    &workspace.connections,
                    &query,
                    ui.get_selected_connection_id().as_str(),
                );
                apply_selected_connection(&ui, &workspace.connections, &selected_id);
            }
        }
    });

    // select-connection
    let ui_weak = ui.as_weak();
    let selection_workspace = Rc::clone(&workspace);
    ui.on_select_connection(move |selected_id| {
        if let Some(ui) = ui_weak.upgrade() {
            let workspace = selection_workspace.borrow();
            apply_selected_connection(&ui, &workspace.connections, selected_id.as_str());
        }
    });

    // connect-selected-connection (opens terminal tab)
    let ui_weak = ui.as_weak();
    let connect_tabs = Rc::clone(&terminal_tabs);
    let connect_active = Rc::clone(&active_tab_index);
    let connect_workspace = Rc::clone(&workspace);
    let connect_settings = Rc::clone(&settings);
    let connect_paths = Rc::clone(&paths);
    let connect_runtime = Rc::clone(&runtime);
    let connect_logs = Rc::clone(&shell_logs);
    ui.on_connect_selected_connection(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let selected_id = ui.get_selected_connection_id().to_string();
            let status = match open_terminal_tab(
                &ui,
                &connect_tabs,
                &connect_active,
                &connect_workspace,
                &connect_settings,
                &connect_paths,
                &connect_runtime,
                &selected_id,
            ) {
                Ok(tab_name) => format!("Opened terminal: {tab_name}"),
                Err(e) => e,
            };
            record_shell_log(&connect_logs, status.clone());
            ui.set_connect_status(status.into());
        }
    });

    // open-connection (opens terminal tab)
    let ui_weak = ui.as_weak();
    let open_tabs = Rc::clone(&terminal_tabs);
    let open_active = Rc::clone(&active_tab_index);
    let open_workspace = Rc::clone(&workspace);
    let open_settings = Rc::clone(&settings);
    let open_paths = Rc::clone(&paths);
    let open_runtime = Rc::clone(&runtime);
    let open_logs = Rc::clone(&shell_logs);
    ui.on_open_connection(move |connection_id| {
        if let Some(ui) = ui_weak.upgrade() {
            let status = match open_terminal_tab(
                &ui,
                &open_tabs,
                &open_active,
                &open_workspace,
                &open_settings,
                &open_paths,
                &open_runtime,
                connection_id.as_str(),
            ) {
                Ok(tab_name) => format!("Opened terminal: {tab_name}"),
                Err(e) => e,
            };
            record_shell_log(&open_logs, status.clone());
            ui.set_connect_status(status.into());
        }
    });

    // select-tab
    let ui_weak = ui.as_weak();
    let select_tabs = Rc::clone(&terminal_tabs);
    let select_active = Rc::clone(&active_tab_index);
    ui.on_select_tab(move |tab_index| {
        if let Some(ui) = ui_weak.upgrade() {
            let tabs = select_tabs.borrow();
            if tab_index >= 0 && (tab_index as usize) < tabs.len() {
                *select_active.borrow_mut() = tab_index;
                ui.set_active_tab_index(tab_index);
                apply_tabs_to_ui(&ui, &tabs, tab_index);
                sync_active_terminal_to_ui(&ui, &tabs[tab_index as usize]);
            }
        }
    });

    // close-tab
    let ui_weak = ui.as_weak();
    let close_tabs = Rc::clone(&terminal_tabs);
    let close_active = Rc::clone(&active_tab_index);
    let close_workspace = Rc::clone(&workspace);
    let close_settings = Rc::clone(&settings);
    let close_logs = Rc::clone(&shell_logs);
    ui.on_close_tab(move |tab_index| {
        if let Some(ui) = ui_weak.upgrade() {
            let mut tabs = close_tabs.borrow_mut();
            if tab_index >= 0 && (tab_index as usize) < tabs.len() {
                let tab = tabs.remove(tab_index as usize);
                tab.disconnect("tab closed");
                let mut active = close_active.borrow_mut();
                if tabs.is_empty() {
                    *active = -1;
                    ui.set_active_tab_index(-1);
                    apply_tabs_to_ui(&ui, &tabs, -1);
                    // Show management view
                    let workspace = close_workspace.borrow();
                    apply_workspace(
                        &ui,
                        &workspace,
                        close_settings.as_ref(),
                        close_logs.borrow().as_slice(),
                        ManageMenu::Connections,
                        "",
                    );
                } else {
                    *active = (*active).min(tabs.len() as i32 - 1);
                    ui.set_active_tab_index(*active);
                    apply_tabs_to_ui(&ui, &tabs, *active);
                    sync_active_terminal_to_ui(&ui, &tabs[*active as usize]);
                }
            }
        }
    });

    // search-changed
    let ui_weak = ui.as_weak();
    let search_workspace = Rc::clone(&workspace);
    let search_settings = Rc::clone(&settings);
    let search_logs = Rc::clone(&shell_logs);
    ui.on_search_changed(move |query| {
        if let Some(ui) = ui_weak.upgrade() {
            let active_menu = manage_menu_from_index(ui.get_active_menu_index());
            let workspace = search_workspace.borrow();
            apply_workspace(
                &ui,
                &workspace,
                search_settings.as_ref(),
                search_logs.borrow().as_slice(),
                active_menu,
                query.as_str(),
            );

            if active_menu == ManageMenu::Connections {
                let selected_id = filtered_selected_connection_id(
                    &workspace.groups,
                    &workspace.connections,
                    query.as_str(),
                    ui.get_selected_connection_id().as_str(),
                );
                apply_selected_connection(&ui, &workspace.connections, &selected_id);
            }
        }
    });

    // terminal-input
    let input_tabs = Rc::clone(&terminal_tabs);
    let input_active = Rc::clone(&active_tab_index);
    ui.on_terminal_input(move |text, alt, control, shift, meta| {
        let active = *input_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = input_tabs.borrow_mut();
        let Some(tab) = tabs.get_mut(active as usize) else {
            return;
        };
        let modifiers = TerminalKeyModifiers {
            alt,
            control,
            shift,
            meta,
        };
        if is_copy_shortcut(&text, modifiers) {
            if let Some(contents) = tab.selected_text() {
                write_clipboard(contents);
            }
            return;
        }
        if is_paste_shortcut(&text, modifiers) {
            let contents = read_clipboard();
            if !contents.is_empty() {
                tab.paste_text(&contents);
            }
            return;
        }
        let Some(payload) = tab.terminal.encode_key_text(&text, modifiers) else {
            return;
        };
        tab.send_terminal_input(payload);
    });

    // terminal-pointer-down
    let pd_tabs = Rc::clone(&terminal_tabs);
    let pd_active = Rc::clone(&active_tab_index);
    ui.on_terminal_pointer_down(move |x, y| {
        let active = *pd_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = pd_tabs.borrow_mut();
        if let Some(tab) = tabs.get_mut(active as usize) {
            tab.pointer_down(x, y);
        }
    });

    // terminal-pointer-moved
    let pm_tabs = Rc::clone(&terminal_tabs);
    let pm_active = Rc::clone(&active_tab_index);
    ui.on_terminal_pointer_moved(move |x, y| {
        let active = *pm_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = pm_tabs.borrow_mut();
        if let Some(tab) = tabs.get_mut(active as usize) {
            tab.pointer_moved(x, y);
        }
    });

    // terminal-pointer-up
    let pu_tabs = Rc::clone(&terminal_tabs);
    let pu_active = Rc::clone(&active_tab_index);
    ui.on_terminal_pointer_up(move |x, y| {
        let active = *pu_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = pu_tabs.borrow_mut();
        if let Some(tab) = tabs.get_mut(active as usize) {
            tab.pointer_up(x, y);
        }
    });

    // terminal-scroll
    let scroll_tabs = Rc::clone(&terminal_tabs);
    let scroll_active = Rc::clone(&active_tab_index);
    ui.on_terminal_scroll(move |delta_y, x, y| {
        let active = *scroll_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = scroll_tabs.borrow_mut();
        if let Some(tab) = tabs.get_mut(active as usize) {
            tab.scroll(delta_y, x, y);
        }
    });

    // terminal-focus-changed
    let fc_tabs = Rc::clone(&terminal_tabs);
    let fc_active = Rc::clone(&active_tab_index);
    ui.on_terminal_focus_changed(move |focused| {
        let active = *fc_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = fc_tabs.borrow_mut();
        if let Some(tab) = tabs.get_mut(active as usize) {
            tab.focus_changed(focused);
        }
    });

    // session-event-ready
    let se_tabs = Rc::clone(&terminal_tabs);
    let se_active = Rc::clone(&active_tab_index);
    let ui_weak = ui.as_weak();
    ui.on_session_event_ready(move || {
        let active = *se_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = se_tabs.borrow_mut();
        if let Some(tab) = tabs.get_mut(active as usize) {
            let dirty = tab.drain_session_events();
            if dirty {
                if let Some(ui) = ui_weak.upgrade() {
                    sync_active_terminal_to_ui(&ui, tab);
                }
            }
        }
    });

    // Timer: update all terminal tabs each frame
    let timer = Timer::default();
    let timer_tabs = Rc::clone(&terminal_tabs);
    let timer_active = Rc::clone(&active_tab_index);
    let ui_weak = ui.as_weak();
    timer.start(TimerMode::Repeated, FRAME_INTERVAL, move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        let active = *timer_active.borrow();
        if active < 0 {
            return;
        }
        let mut tabs = timer_tabs.borrow_mut();
        if let Some(tab) = tabs.get_mut(active as usize) {
            let now = Instant::now();
            let dirty = sync_terminal_tab_layout(&ui, tab);
            let dirty = dirty || tab.drain_terminal_events();
            let dirty = dirty || tab.update_cursor_blink(now);
            if dirty {
                sync_active_terminal_to_ui(&ui, tab);
            }
        }
    });

    let run_result = ui.run();

    // Cleanup: disconnect all terminal tabs
    {
        let tabs = terminal_tabs.borrow();
        for tab in tabs.iter() {
            tab.disconnect("窗口关闭");
        }
    }
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(50)).await;
    });
    // Runtime will be dropped when all Rc references are dropped
    drop(runtime);

    run_result.map_err(Into::into)
}

// --- Terminal tab management ---

fn open_terminal_tab(
    ui: &TimonSlintShellWindow,
    tabs: &Rc<RefCell<Vec<TerminalTab>>>,
    active_index: &Rc<RefCell<i32>>,
    workspace: &Rc<RefCell<workspace::WorkspaceData>>,
    settings: &Rc<AppSettings>,
    paths: &Rc<AppPaths>,
    runtime: &Rc<tokio::runtime::Runtime>,
    connection_id_str: &str,
) -> Result<String, String> {
    let connection_id: i64 = connection_id_str
        .parse()
        .map_err(|_| "Invalid connection ID".to_string())?;

    let ws = workspace.borrow();
    let connection = ws
        .connections
        .iter()
        .find(|c| c.id == connection_id)
        .ok_or_else(|| "Connection not found".to_string())?
        .clone();
    let key = ws
        .keys
        .iter()
        .find(|k| Some(k.id) == connection.key_id)
        .cloned();
    let identity = ws
        .identities
        .iter()
        .find(|i| Some(i.id) == connection.identity_id)
        .cloned();
    let known_hosts_path = paths.known_hosts.clone();
    let terminal_themes = load_custom_terminal_themes(&paths.themes);

    let mut terminal_settings = settings.terminal.clone();
    terminal_settings.colors = slint_terminal_colors_for_connection(
        &terminal_settings,
        &terminal_themes,
        &connection.theme_id,
    );

    let terminal_line_height = terminal_settings.font.line_height;
    let terminal_theme = TerminalTheme::from_settings(&terminal_settings.colors);
    let terminal_font = TerminalFont::from_settings(&terminal_settings.font);
    let mut terminal_view = TerminalView::new(
        TERMINAL_COLS as usize,
        TERMINAL_ROWS as usize,
        &terminal_settings,
    );

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let pending_session_events = Arc::new(Mutex::new(VecDeque::new()));

    let session = runtime
        .block_on(connect_target(
            ConnectionTarget {
                connection: connection.clone(),
                key,
                identity,
                known_hosts_path,
                cols: TERMINAL_COLS,
                rows: TERMINAL_ROWS,
            },
            event_tx,
        ))
        .map_err(|e| format!("Connection failed: {e}"))?;

    terminal_view.set_outbound(session.command_tx.clone());

    let tab_name = connection.name.clone();
    let tab_id = format!("terminal-{connection_id}");
    let tab = TerminalTab::new(
        tab_id.clone(),
        tab_name.clone(),
        terminal_view,
        session,
        terminal_theme,
        terminal_font,
        terminal_line_height,
        Arc::clone(&pending_session_events),
        tab_name.clone(),
    );

    // Spawn session event forwarder
    spawn_session_event_forwarder(runtime, event_rx, pending_session_events, ui.as_weak());

    let mut tabs_mut = tabs.borrow_mut();
    let new_index = tabs_mut.len() as i32;
    tabs_mut.push(tab);
    *active_index.borrow_mut() = new_index;
    ui.set_active_tab_index(new_index);
    apply_tabs_to_ui(ui, &tabs_mut, new_index);
    sync_active_terminal_to_ui(ui, &tabs_mut[new_index as usize]);

    Ok(tab_name)
}

fn apply_tabs_to_ui(ui: &TimonSlintShellWindow, tabs: &[TerminalTab], active_index: i32) {
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| TabItem {
            id: tab.id.clone().into(),
            title: tab.window_title.clone().into(),
            active: i as i32 == active_index,
            is_terminal: true,
        })
        .collect();
    ui.set_tabs(slint::ModelRc::new(slint::VecModel::from(tab_items)));
}

fn sync_active_terminal_to_ui(ui: &TimonSlintShellWindow, tab: &TerminalTab) {
    let snapshot = tab.terminal.snapshot(&tab.theme);
    let cursor_visible = tab.focused && tab.cursor_visible;
    let overlay = cursor_overlay_from_snapshot(&snapshot, &tab.font, cursor_visible);

    ui.set_terminal_background(slint_color(tab.theme.background.rgba8()));
    ui.set_terminal_font_family(tab.font.family_name.clone().into());
    ui.set_terminal_font_size(tab.font.size);
    ui.set_terminal_cells(snapshot_to_shell_cells(
        &snapshot,
        tab.selection.as_ref(),
        &tab.font,
        cursor_visible,
    ));
    ui.set_terminal_decorations(snapshot_to_shell_decorations(&snapshot, &tab.font));
    ui.set_cursor_overlay_visible(overlay.visible);
    ui.set_cursor_overlay_x(overlay.x);
    ui.set_cursor_overlay_y(overlay.y);
    ui.set_cursor_overlay_width(overlay.width);
    ui.set_cursor_overlay_height(overlay.height);
    ui.set_cursor_overlay_color(slint_color(overlay.color.rgba8()));

    // Update tab title in the tab bar
    apply_tabs_to_ui(ui, &std::slice::from_ref(tab), 0);
}

fn sync_terminal_tab_layout(ui: &TimonSlintShellWindow, tab: &mut TerminalTab) -> bool {
    let window = ui.window();
    let size = window.size();
    let scale_factor = window.scale_factor();
    let content_width = size.width.saturating_sub(184);
    let tab_bar_height: u32 = 36;
    let content_height = size.height.saturating_sub(52 + tab_bar_height);

    let native_w = ui.get_terminal_native_cell_width();
    let native_h = ui.get_terminal_native_cell_height();
    let mut dirty = tab.sync_font_metrics(native_w, native_h);
    dirty |= tab.sync_window_size(content_width, content_height, scale_factor);
    dirty
}

fn spawn_session_event_forwarder(
    runtime: &tokio::runtime::Runtime,
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<SessionEvent>,
    pending_session_events: Arc<Mutex<VecDeque<SessionEvent>>>,
    ui_weak: slint::Weak<TimonSlintShellWindow>,
) {
    runtime.spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if let Ok(mut pending) = pending_session_events.lock() {
                pending.push_back(event);
            }
            let ui_weak = ui_weak.clone();
            let _ = ui_weak.upgrade_in_event_loop(|ui| {
                ui.invoke_session_event_ready();
            });
        }
    });
}

fn slint_terminal_colors_for_connection(
    settings: &TerminalSettings,
    themes: &[TerminalThemeEntry],
    theme_id: &str,
) -> TerminalColors {
    let id = if theme_id.is_empty() {
        &settings.default_theme_id
    } else {
        theme_id
    };
    if let Some(entry) = builtin_terminal_theme_by_id(id) {
        return entry.colors.clone();
    }
    if let Some(entry) = themes.iter().find(|t| t.id == *id) {
        return entry.colors.clone();
    }
    settings.colors.clone()
}

fn apply_workspace(
    ui: &TimonSlintShellWindow,
    workspace: &workspace::WorkspaceData,
    settings: &AppSettings,
    logs: &[String],
    active_menu: ManageMenu,
    search_query: &str,
) {
    ui.set_nav_items(model(nav_items(active_menu)));
    ui.set_stats(model(stat_items(workspace, settings, logs, active_menu)));
    let groups = connection_group_items(&workspace.groups, &workspace.connections, search_query);
    let group_rows = groups.len().div_ceil(2);
    ui.set_group_items(model(groups));
    ui.set_connections(model(connection_items(
        &filtered_connections(&workspace.groups, &workspace.connections, search_query),
        &workspace.groups,
    )));
    ui.set_list_items(model(list_items(
        workspace,
        settings,
        logs,
        active_menu,
        search_query,
    )));
    ui.set_active_menu_index(active_menu.index() as i32);
    ui.set_connection_group_rows(group_rows as i32);
    ui.set_page_title(active_menu.title().into());
    ui.set_page_subtitle(page_subtitle(active_menu).into());
    ui.set_search_query(search_query.into());
    ui.set_connect_status(String::new().into());
    ui.set_connection_count_label(format!("{} connections", workspace.connections.len()).into());
    ui.set_group_count_label(format!("{} groups", workspace.groups.len()).into());
}

fn launch_status_for_active_menu(active_menu: ManageMenu, _selected_connection_id: &str) -> String {
    if active_menu != ManageMenu::Connections {
        return "Switch to Connections to open a terminal".into();
    }
    "Opening terminal...".into()
}

fn nav_items(active_menu: ManageMenu) -> Vec<ShellNavItem> {
    ManageMenu::ALL
        .iter()
        .map(|menu| ShellNavItem {
            index: menu.index() as i32,
            title: menu.title().into(),
            active: *menu == active_menu,
        })
        .collect()
}

fn manage_menu_from_index(index: i32) -> ManageMenu {
    ManageMenu::ALL
        .get(index.max(0) as usize)
        .copied()
        .unwrap_or(ManageMenu::Connections)
}

fn page_subtitle(active_menu: ManageMenu) -> &'static str {
    match active_menu {
        ManageMenu::Connections => "Saved SSH targets, local shells, and serial sessions.",
        ManageMenu::Keychain => "Keys and identities available to connection profiles.",
        ManageMenu::PortForwarding => "Local, remote, and dynamic forwarding rules.",
        ManageMenu::Snippets => "Reusable commands and operational playbook shortcuts.",
        ManageMenu::KnownHosts => "Hosts trusted by the SSH known_hosts store.",
        ManageMenu::Logs => "Connection and system events will be collected here.",
        ManageMenu::Settings => "Workspace-level preferences and runtime configuration.",
    }
}

fn stat_items(
    workspace: &workspace::WorkspaceData,
    settings: &AppSettings,
    logs: &[String],
    active_menu: ManageMenu,
) -> Vec<ShellStatItem> {
    match active_menu {
        ManageMenu::Connections => connection_stat_items(workspace),
        ManageMenu::Keychain => vec![
            stat_item("Keys", workspace.keys.len(), "stored keys"),
            stat_item("Identities", workspace.identities.len(), "login profiles"),
            stat_item(
                "Total",
                workspace.keys.len() + workspace.identities.len(),
                "credentials",
            ),
        ],
        ManageMenu::PortForwarding => {
            let enabled = workspace
                .port_forwards
                .iter()
                .filter(|forward| forward.enabled)
                .count();
            vec![
                stat_item("Rules", workspace.port_forwards.len(), "configured"),
                stat_item("Enabled", enabled, "active rules"),
                stat_item(
                    "Disabled",
                    workspace.port_forwards.len().saturating_sub(enabled),
                    "paused rules",
                ),
            ]
        }
        ManageMenu::KnownHosts => vec![
            stat_item(
                "Known Hosts",
                workspace.known_hosts.len(),
                "trusted entries",
            ),
            stat_item("Groups", workspace.groups.len(), "connection groups"),
            stat_item("Connections", workspace.connections.len(), "saved targets"),
        ],
        ManageMenu::Settings => vec![
            stat_text(
                "Theme",
                &settings.terminal.default_theme_id,
                "default terminal theme",
            ),
            stat_text(
                "Font",
                &format!("{:.0}px", settings.terminal.font.size),
                settings.terminal.font.family.as_str(),
            ),
            stat_item(
                "Scrollback",
                settings.terminal.scrollback_lines,
                "terminal lines",
            ),
        ],
        ManageMenu::Logs => vec![
            stat_item("Events", logs.len(), "in-memory shell logs"),
            stat_text(
                "Latest",
                logs.last().map(String::as_str).unwrap_or("No events"),
                "most recent event",
            ),
            stat_item("Limit", SHELL_LOG_LIMIT, "max entries"),
        ],
        ManageMenu::Snippets => {
            let ready = workspace
                .snippets
                .iter()
                .filter(|snippet| !snippet.command.trim().is_empty())
                .count();
            vec![
                stat_item("Snippets", workspace.snippets.len(), "saved commands"),
                stat_item("Ready", ready, "with command"),
                stat_item(
                    "Drafts",
                    workspace.snippets.len().saturating_sub(ready),
                    "missing command",
                ),
            ]
        }
    }
}

fn connection_stat_items(workspace: &workspace::WorkspaceData) -> Vec<ShellStatItem> {
    let ssh_count = workspace
        .connections
        .iter()
        .filter(|connection| connection.connection_type == ConnectionType::Ssh)
        .count();
    let local_count = workspace
        .connections
        .iter()
        .filter(|connection| connection.connection_type == ConnectionType::Local)
        .count();

    vec![
        stat_item("Connections", workspace.connections.len(), "saved targets"),
        stat_item("SSH", ssh_count, "remote hosts"),
        stat_item("Local", local_count, "shell profiles"),
    ]
}

fn stat_item(title: &str, value: usize, caption: &str) -> ShellStatItem {
    stat_text(title, &value.to_string(), caption)
}

fn stat_text(title: &str, value: &str, caption: &str) -> ShellStatItem {
    ShellStatItem {
        title: title.into(),
        value: value.into(),
        caption: caption.into(),
    }
}

fn connection_items(connections: &[Connection], groups: &[Group]) -> Vec<ShellConnectionItem> {
    connections
        .iter()
        .map(|connection| ShellConnectionItem {
            id: connection.id.to_string().into(),
            name: connection.name.clone().into(),
            endpoint: connection_summary(connection, groups).into(),
            badge: connection_type_label(connection.connection_type).into(),
            initial: connection_initial(&connection.name).into(),
            accent: connection_accent(connection.connection_type),
        })
        .collect()
}

fn connection_group_items(
    groups: &[Group],
    connections: &[Connection],
    search_query: &str,
) -> Vec<ShellListItem> {
    let items = groups
        .iter()
        .filter(|group| search_matches(search_query, [group.name.as_str()]))
        .map(|group| {
            let connection_count = connections
                .iter()
                .filter(|connection| connection.group_id == Some(group.id))
                .count();
            let parent = group
                .parent_id
                .and_then(|parent_id| groups.iter().find(|candidate| candidate.id == parent_id))
                .map(|parent| parent.name.as_str())
                .unwrap_or("Root");
            shell_list_item(
                format!("group-{}", group.id),
                group.name.clone(),
                format!("{connection_count} connections / parent {parent}"),
                "GROUP",
                connection_initial(&group.name),
                slint::Color::from_rgb_u8(0, 92, 145),
            )
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        placeholder_items("Groups", "No groups found in the current workspace.")
    } else {
        items
    }
}

fn filtered_connections(
    groups: &[Group],
    connections: &[Connection],
    search_query: &str,
) -> Vec<Connection> {
    connections
        .iter()
        .filter(|connection| {
            let endpoint = connection_endpoint(connection);
            let group_name = connection_group_name(connection, groups).unwrap_or_default();
            search_matches(
                search_query,
                [
                    connection.name.as_str(),
                    connection.host.as_str(),
                    connection.display_username.as_str(),
                    connection.username.as_str(),
                    connection.serial_port.as_str(),
                    endpoint.as_str(),
                    group_name.as_str(),
                ],
            )
        })
        .cloned()
        .collect()
}

fn filtered_selected_connection_id(
    groups: &[Group],
    connections: &[Connection],
    search_query: &str,
    current_selected_id: &str,
) -> String {
    let filtered = filtered_connections(groups, connections, search_query);
    if filtered
        .iter()
        .any(|connection| connection.id.to_string() == current_selected_id)
    {
        return current_selected_id.into();
    }

    initial_selected_connection_id(&filtered)
}

fn list_items(
    workspace: &workspace::WorkspaceData,
    settings: &AppSettings,
    logs: &[String],
    active_menu: ManageMenu,
    search_query: &str,
) -> Vec<ShellListItem> {
    let items = match active_menu {
        ManageMenu::Connections => Vec::new(),
        ManageMenu::Keychain => filter_list_items(
            keychain_items(&workspace.keys, &workspace.identities),
            search_query,
        ),
        ManageMenu::PortForwarding => {
            filter_list_items(port_forward_items(&workspace.port_forwards), search_query)
        }
        ManageMenu::KnownHosts => {
            filter_list_items(known_host_items(&workspace.known_hosts), search_query)
        }
        ManageMenu::Snippets => filter_list_items(snippet_items(&workspace.snippets), search_query),
        ManageMenu::Logs => filter_list_items(log_items(logs), search_query),
        ManageMenu::Settings => filter_list_items(settings_items(settings), search_query),
    };

    if items.is_empty() && active_menu != ManageMenu::Connections {
        return placeholder_items(
            active_menu.title(),
            "No records found in the current workspace.",
        );
    }

    items
}

fn initial_shell_logs(workspace: &workspace::WorkspaceData, settings: &AppSettings) -> Vec<String> {
    vec![
        "Timon Slint shell started".into(),
        format!(
            "Loaded {} connections, {} keys, {} identities",
            workspace.connections.len(),
            workspace.keys.len(),
            workspace.identities.len()
        ),
        format!(
            "Loaded {} port forwards and {} known hosts",
            workspace.port_forwards.len(),
            workspace.known_hosts.len()
        ),
        format!(
            "Settings loaded: theme {}, font {} {:.0}px",
            settings.terminal.default_theme_id,
            settings.terminal.font.family,
            settings.terminal.font.size
        ),
    ]
}

fn record_shell_log(logs: &Rc<RefCell<Vec<String>>>, message: impl Into<String>) {
    let mut logs = logs.borrow_mut();
    logs.push(message.into());
    if logs.len() > SHELL_LOG_LIMIT {
        let overflow = logs.len() - SHELL_LOG_LIMIT;
        logs.drain(0..overflow);
    }
}

fn shell_list_item(
    id: impl Into<slint::SharedString>,
    title: impl Into<slint::SharedString>,
    subtitle: impl Into<slint::SharedString>,
    badge: impl Into<slint::SharedString>,
    initial: impl Into<slint::SharedString>,
    accent: slint::Color,
) -> ShellListItem {
    ShellListItem {
        id: id.into(),
        title: title.into(),
        subtitle: subtitle.into(),
        badge: badge.into(),
        initial: initial.into(),
        accent,
    }
}

fn log_items(logs: &[String]) -> Vec<ShellListItem> {
    logs.iter()
        .rev()
        .enumerate()
        .map(|(index, message)| {
            let number = logs.len().saturating_sub(index);
            shell_list_item(
                format!("log-{number}"),
                format!("#{number}"),
                message.clone(),
                "LOG",
                "L",
                slint::Color::from_rgb_u8(35, 150, 165),
            )
        })
        .collect()
}

fn settings_items(settings: &AppSettings) -> Vec<ShellListItem> {
    vec![
        shell_list_item(
            "settings-font",
            "Terminal Font",
            format!(
                "{} / {:.0}px / line height {:.2}",
                settings.terminal.font.family,
                settings.terminal.font.size,
                settings.terminal.font.line_height
            ),
            "FONT",
            "F",
            slint::Color::from_rgb_u8(37, 99, 235),
        ),
        shell_list_item(
            "settings-theme",
            "Default Theme",
            settings.terminal.default_theme_id.clone(),
            "THEME",
            "T",
            slint::Color::from_rgb_u8(132, 90, 223),
        ),
        shell_list_item(
            "settings-scrollback",
            "Scrollback",
            format!("{} lines", settings.terminal.scrollback_lines),
            "TERM",
            "S",
            slint::Color::from_rgb_u8(35, 150, 165),
        ),
        shell_list_item(
            "settings-cursor",
            "Cursor",
            format!(
                "{} / {}",
                settings.terminal.cursor.shape,
                if settings.terminal.cursor.blinking {
                    "blinking"
                } else {
                    "steady"
                }
            ),
            "CURSOR",
            "C",
            slint::Color::from_rgb_u8(244, 94, 52),
        ),
        shell_list_item(
            "settings-shortcuts",
            "Shortcuts",
            format!(
                "close {} / settings {}",
                settings.shortcuts.close_tab, settings.shortcuts.open_settings
            ),
            "KEYS",
            "K",
            slint::Color::from_rgb_u8(127, 138, 150),
        ),
    ]
}

fn filter_list_items(items: Vec<ShellListItem>, search_query: &str) -> Vec<ShellListItem> {
    items
        .into_iter()
        .filter(|item| {
            search_matches(
                search_query,
                [
                    item.title.as_str(),
                    item.subtitle.as_str(),
                    item.badge.as_str(),
                ],
            )
        })
        .collect()
}

fn search_matches<'a>(search_query: &str, values: impl IntoIterator<Item = &'a str>) -> bool {
    let query = search_query.trim().to_lowercase();
    query.is_empty()
        || values
            .into_iter()
            .any(|value| value.to_lowercase().contains(&query))
}

fn keychain_items(keys: &[SshKey], identities: &[Identity]) -> Vec<ShellListItem> {
    keys.iter()
        .map(|key| {
            shell_list_item(
                format!("key-{}", key.id),
                key.name.clone(),
                key_fingerprint_preview(key),
                "KEY",
                connection_initial(&key.name),
                slint::Color::from_rgb_u8(37, 99, 235),
            )
        })
        .chain(identities.iter().map(|identity| {
            shell_list_item(
                format!("identity-{}", identity.id),
                identity.name.clone(),
                identity_subtitle(identity),
                "ID",
                connection_initial(&identity.name),
                slint::Color::from_rgb_u8(132, 90, 223),
            )
        }))
        .collect()
}

fn key_fingerprint_preview(key: &SshKey) -> String {
    if !key.public_key.trim().is_empty() {
        return key.public_key.trim().chars().take(42).collect();
    }

    if !key.certificate.trim().is_empty() {
        return "Certificate attached".into();
    }

    "Private key".into()
}

fn identity_subtitle(identity: &Identity) -> String {
    if identity.username.trim().is_empty() {
        "No username".into()
    } else {
        identity.username.trim().into()
    }
}

fn port_forward_items(port_forwards: &[PortForward]) -> Vec<ShellListItem> {
    port_forwards
        .iter()
        .map(|forward| {
            let title = if forward.label.trim().is_empty() {
                "Forward".into()
            } else {
                forward.label.clone()
            };
            shell_list_item(
                format!("port-forward-{}", forward.id),
                title,
                port_forward_subtitle(forward),
                if forward.enabled { "ON" } else { "OFF" },
                connection_initial(&forward.label),
                slint::Color::from_rgb_u8(244, 94, 52),
            )
        })
        .collect()
}

fn port_forward_subtitle(forward: &PortForward) -> String {
    format!(
        "{} {}:{} -> {}:{}",
        forward.forward_type.label(),
        forward.bind_address,
        forward.bind_port,
        forward.destination_host,
        forward.destination_port
    )
}

fn snippet_items(snippets: &[Snippet]) -> Vec<ShellListItem> {
    snippets
        .iter()
        .map(|snippet| {
            let title = if snippet.name.trim().is_empty() {
                "Snippet".into()
            } else {
                snippet.name.clone()
            };
            shell_list_item(
                snippet.id.to_string(),
                title,
                snippet_subtitle(snippet),
                "SNIP",
                connection_initial(&snippet.name),
                slint::Color::from_rgb_u8(245, 158, 11),
            )
        })
        .collect()
}

fn snippet_subtitle(snippet: &Snippet) -> String {
    let description = snippet.description.trim();
    let command = snippet.command.trim();

    match (description.is_empty(), command.is_empty()) {
        (false, false) => format!("{description} / {command}"),
        (false, true) => description.into(),
        (true, false) => command.into(),
        (true, true) => "No command".into(),
    }
}

fn known_host_items(known_hosts: &[KnownHostEntry]) -> Vec<ShellListItem> {
    known_hosts
        .iter()
        .map(|entry| {
            let title = entry
                .line
                .split_whitespace()
                .next()
                .filter(|host| !host.trim().is_empty())
                .unwrap_or("Known Host");

            shell_list_item(
                format!("known-host-{}", entry.line_number),
                title,
                format!("line {}", entry.line_number),
                "HOST",
                connection_initial(title),
                slint::Color::from_rgb_u8(35, 150, 165),
            )
        })
        .collect()
}

fn placeholder_items(title: &str, subtitle: &str) -> Vec<ShellListItem> {
    vec![shell_list_item(
        "placeholder",
        title,
        subtitle,
        "EMPTY",
        connection_initial(title),
        slint::Color::from_rgb_u8(127, 138, 150),
    )]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedConnectionDetails {
    id: String,
    name: String,
    endpoint: String,
    connection_type: String,
}

fn initial_selected_connection_id(connections: &[Connection]) -> String {
    connections
        .first()
        .map(|connection| connection.id.to_string())
        .unwrap_or_default()
}

fn selected_connection_details(
    connections: &[Connection],
    selected_id: &str,
) -> SelectedConnectionDetails {
    let selected = connections
        .iter()
        .find(|connection| connection.id.to_string() == selected_id)
        .or_else(|| connections.first());

    if let Some(connection) = selected {
        return SelectedConnectionDetails {
            id: connection.id.to_string(),
            name: connection.name.clone(),
            endpoint: connection_endpoint(connection),
            connection_type: connection_type_label(connection.connection_type).to_string(),
        };
    }

    SelectedConnectionDetails {
        id: String::new(),
        name: "No Connection".into(),
        endpoint: "Create or import a connection to get started.".into(),
        connection_type: "EMPTY".into(),
    }
}

fn apply_selected_connection(
    ui: &TimonSlintShellWindow,
    connections: &[Connection],
    selected_id: &str,
) {
    let details = selected_connection_details(connections, selected_id);

    ui.set_selected_connection_id(details.id.into());
    ui.set_selected_connection_name(details.name.into());
    ui.set_selected_connection_endpoint(details.endpoint.into());
    ui.set_selected_connection_type(details.connection_type.into());
}

fn connection_summary(connection: &Connection, groups: &[Group]) -> String {
    let endpoint = connection_endpoint(connection);
    match connection_group_name(connection, groups) {
        Some(group) => format!("{endpoint} / group {group}"),
        None => endpoint,
    }
}

fn connection_group_name(connection: &Connection, groups: &[Group]) -> Option<String> {
    connection
        .group_id
        .and_then(|group_id| groups.iter().find(|group| group.id == group_id))
        .map(|group| group.name.clone())
}

fn connection_endpoint(connection: &Connection) -> String {
    match connection.connection_type {
        ConnectionType::Local => {
            let shell = if connection.shell_path.trim().is_empty() {
                "Login Shell"
            } else {
                connection.shell_path.trim()
            };
            let work_dir = if connection.work_dir.trim().is_empty() {
                "Home"
            } else {
                connection.work_dir.trim()
            };
            format!("{shell} / {work_dir}")
        }
        ConnectionType::Serial => {
            let port = if connection.serial_port.trim().is_empty() {
                "Serial Port"
            } else {
                connection.serial_port.trim()
            };
            format!("{port} / {}", connection.baud_rate)
        }
        ConnectionType::Ssh => {
            let username = if connection.display_username.trim().is_empty() {
                "ssh"
            } else {
                connection.display_username.trim()
            };
            let host = if connection.host.trim().is_empty() {
                "hostname"
            } else {
                connection.host.trim()
            };
            format!("{username}@{host}:{}", connection.port)
        }
    }
}

fn connection_type_label(connection_type: ConnectionType) -> &'static str {
    match connection_type {
        ConnectionType::Ssh => "SSH",
        ConnectionType::Local => "LOCAL",
        ConnectionType::Serial => "SERIAL",
    }
}

fn connection_initial(name: &str) -> String {
    name.trim()
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "T".into())
}

fn connection_accent(connection_type: ConnectionType) -> slint::Color {
    match connection_type {
        ConnectionType::Ssh => slint::Color::from_rgb_u8(55, 204, 143),
        ConnectionType::Local => slint::Color::from_rgb_u8(37, 99, 235),
        ConnectionType::Serial => slint::Color::from_rgb_u8(244, 94, 52),
    }
}

fn model<T>(items: Vec<T>) -> slint::ModelRc<T>
where
    T: Clone + 'static,
{
    slint::ModelRc::new(slint::VecModel::from(items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_endpoint_uses_display_username_host_and_port() {
        let connection = Connection {
            display_username: "admin".into(),
            host: "10.10.1.110".into(),
            port: 2022,
            connection_type: ConnectionType::Ssh,
            ..Connection::default()
        };

        assert_eq!(connection_endpoint(&connection), "admin@10.10.1.110:2022");
    }

    #[test]
    fn local_endpoint_falls_back_to_login_shell_and_home() {
        let connection = Connection {
            connection_type: ConnectionType::Local,
            ..Connection::default()
        };

        assert_eq!(connection_endpoint(&connection), "Login Shell / Home");
    }

    #[test]
    fn connection_initial_uses_first_alphanumeric_character() {
        assert_eq!(connection_initial("  chore"), "C");
        assert_eq!(connection_initial("!!!"), "T");
    }

    #[test]
    fn manage_menu_from_index_falls_back_safely() {
        assert_eq!(manage_menu_from_index(1), ManageMenu::Keychain);
        assert_eq!(manage_menu_from_index(-1), ManageMenu::Connections);
        assert_eq!(manage_menu_from_index(99), ManageMenu::Connections);
    }

    #[test]
    fn launch_status_rejects_non_connection_pages_before_spawning() {
        assert_eq!(
            launch_status_for_active_menu(ManageMenu::Keychain, "7"),
            "Switch to Connections to open a terminal"
        );
    }

    #[test]
    fn launch_status_accepts_connections_page_before_opening_tab() {
        assert_eq!(
            launch_status_for_active_menu(ManageMenu::Connections, ""),
            "Opening terminal..."
        );
    }

    #[test]
    fn nav_items_marks_active_menu() {
        let items = nav_items(ManageMenu::KnownHosts);

        assert_eq!(items.len(), ManageMenu::ALL.len());
        assert!(items[ManageMenu::KnownHosts.index()].active);
        assert!(!items[ManageMenu::Connections.index()].active);
        assert_eq!(
            items[ManageMenu::KnownHosts.index()].title.to_string(),
            "Known Hosts"
        );
    }

    #[test]
    fn keychain_items_include_keys_and_identities() {
        let items = keychain_items(
            &[SshKey {
                id: 11,
                name: "prod_ed25519".into(),
                public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK".into(),
                ..SshKey::default()
            }],
            &[Identity {
                id: 22,
                name: "deploy".into(),
                username: "admin".into(),
                ..Identity::default()
            }],
        );

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.to_string(), "key-11");
        assert_eq!(items[0].title.to_string(), "prod_ed25519");
        assert_eq!(items[0].badge.to_string(), "KEY");
        assert_eq!(items[1].id.to_string(), "identity-22");
        assert_eq!(items[1].title.to_string(), "deploy");
        assert_eq!(items[1].subtitle.to_string(), "admin");
        assert_eq!(items[1].badge.to_string(), "ID");
    }

    #[test]
    fn port_forward_items_describe_route_and_enabled_state() {
        let items = port_forward_items(&[
            PortForward {
                id: 7,
                label: "redis".into(),
                enabled: true,
                bind_address: "127.0.0.1".into(),
                bind_port: 6379,
                destination_host: "10.0.0.10".into(),
                destination_port: 6379,
                ..PortForward::default()
            },
            PortForward {
                id: 8,
                label: "api".into(),
                enabled: false,
                ..PortForward::default()
            },
        ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.to_string(), "port-forward-7");
        assert_eq!(items[0].title.to_string(), "redis");
        assert_eq!(
            items[0].subtitle.to_string(),
            "Local 127.0.0.1:6379 -> 10.0.0.10:6379"
        );
        assert_eq!(items[0].badge.to_string(), "ON");
        assert_eq!(items[1].badge.to_string(), "OFF");
    }

    #[test]
    fn snippet_items_include_description_and_command() {
        let items = snippet_items(&[Snippet {
            id: 42,
            name: "Restart API".into(),
            description: "Production restart".into(),
            command: "systemctl restart api".into(),
        }]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id.to_string(), "42");
        assert_eq!(items[0].title.to_string(), "Restart API");
        assert_eq!(
            items[0].subtitle.to_string(),
            "Production restart / systemctl restart api"
        );
        assert_eq!(items[0].badge.to_string(), "SNIP");
    }

    #[test]
    fn snippets_page_filters_by_command_text() {
        let workspace = workspace::WorkspaceData {
            snippets: vec![
                Snippet {
                    name: "Restart API".into(),
                    command: "systemctl restart api".into(),
                    ..Snippet::default()
                },
                Snippet {
                    name: "Tail logs".into(),
                    command: "journalctl -fu timon".into(),
                    ..Snippet::default()
                },
            ],
            ..workspace::WorkspaceData::default()
        };
        let settings = AppSettings::default();
        let logs = Vec::new();

        let items = list_items(
            &workspace,
            &settings,
            &logs,
            ManageMenu::Snippets,
            "journalctl",
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Tail logs");
    }

    #[test]
    fn known_host_items_use_host_pattern_and_line_number() {
        let items = known_host_items(&[KnownHostEntry {
            line_number: 12,
            line: "example.com ssh-ed25519 AAAA".into(),
        }]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "example.com");
        assert_eq!(items[0].subtitle.to_string(), "line 12");
        assert_eq!(items[0].badge.to_string(), "HOST");
    }

    #[test]
    fn list_items_returns_empty_state_for_empty_migrated_page() {
        let workspace = workspace::WorkspaceData::default();
        let settings = AppSettings::default();
        let logs = Vec::new();
        let items = list_items(&workspace, &settings, &logs, ManageMenu::Keychain, "");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Keychain");
        assert_eq!(items[0].badge.to_string(), "EMPTY");
    }

    #[test]
    fn filtered_connections_matches_name_host_and_endpoint() {
        let groups = vec![Group {
            id: 10,
            name: "Production".into(),
            parent_id: None,
        }];
        let connections = vec![
            Connection {
                id: 1,
                name: "Local Shell".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
            Connection {
                id: 2,
                name: "Chore".into(),
                display_username: "admin".into(),
                host: "10.10.1.110".into(),
                port: 2022,
                group_id: Some(10),
                connection_type: ConnectionType::Ssh,
                ..Connection::default()
            },
        ];

        assert_eq!(
            filtered_connections(&groups, &connections, "chore").len(),
            1
        );
        assert_eq!(
            filtered_connections(&groups, &connections, "10.10.1.110")[0].id,
            2
        );
        assert_eq!(
            filtered_connections(&groups, &connections, "admin@10.10")[0].id,
            2
        );
        assert_eq!(
            filtered_connections(&groups, &connections, "production")[0].id,
            2
        );
    }

    #[test]
    fn connection_items_include_group_name_in_summary() {
        let groups = vec![Group {
            id: 10,
            name: "Production".into(),
            parent_id: None,
        }];
        let connections = vec![Connection {
            name: "Chore".into(),
            display_username: "admin".into(),
            host: "10.10.1.110".into(),
            port: 2022,
            group_id: Some(10),
            connection_type: ConnectionType::Ssh,
            ..Connection::default()
        }];

        let items = connection_items(&connections, &groups);

        assert_eq!(
            items[0].endpoint.to_string(),
            "admin@10.10.1.110:2022 / group Production"
        );
    }

    #[test]
    fn connection_group_items_include_counts_and_parent_names() {
        let groups = vec![
            Group {
                id: 1,
                name: "Production".into(),
                parent_id: None,
            },
            Group {
                id: 2,
                name: "Web".into(),
                parent_id: Some(1),
            },
        ];
        let connections = vec![
            Connection {
                group_id: Some(2),
                ..Connection::default()
            },
            Connection {
                group_id: Some(2),
                ..Connection::default()
            },
        ];

        let items = connection_group_items(&groups, &connections, "web");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Web");
        assert_eq!(
            items[0].subtitle.to_string(),
            "2 connections / parent Production"
        );
        assert_eq!(items[0].badge.to_string(), "GROUP");
    }

    #[test]
    fn connection_group_items_return_empty_state_without_groups() {
        let items = connection_group_items(&[], &[], "");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Groups");
        assert_eq!(items[0].badge.to_string(), "EMPTY");
    }

    #[test]
    fn filtered_selected_connection_falls_back_to_first_visible_result() {
        let connections = vec![
            Connection {
                id: 1,
                name: "alpha".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
            Connection {
                id: 2,
                name: "beta".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
        ];

        assert_eq!(
            filtered_selected_connection_id(&[], &connections, "beta", "1"),
            "2"
        );
        assert_eq!(
            filtered_selected_connection_id(&[], &connections, "gamma", "1"),
            ""
        );
    }

    #[test]
    fn list_items_filters_current_panel_records() {
        let workspace = workspace::WorkspaceData {
            keys: vec![SshKey {
                name: "prod_ed25519".into(),
                public_key: "ssh-ed25519 AAAAC3".into(),
                ..SshKey::default()
            }],
            identities: vec![Identity {
                name: "staging".into(),
                username: "deploy".into(),
                ..Identity::default()
            }],
            ..workspace::WorkspaceData::default()
        };
        let settings = AppSettings::default();

        let logs = Vec::new();
        let items = list_items(&workspace, &settings, &logs, ManageMenu::Keychain, "deploy");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "staging");
    }

    #[test]
    fn settings_items_include_terminal_and_shortcut_summary() {
        let mut settings = AppSettings::default();
        settings.terminal.font.family = "JetBrains Mono".into();
        settings.terminal.font.size = 13.0;
        settings.terminal.default_theme_id = "atom-one-dark".into();
        settings.shortcuts.open_settings = "Command+,".into();

        let items = settings_items(&settings);

        assert_eq!(items.len(), 5);
        assert_eq!(items[0].title.to_string(), "Terminal Font");
        assert!(items[0].subtitle.to_string().contains("JetBrains Mono"));
        assert_eq!(items[1].title.to_string(), "Default Theme");
        assert_eq!(items[1].subtitle.to_string(), "atom-one-dark");
        assert_eq!(items[4].title.to_string(), "Shortcuts");
        assert!(items[4].subtitle.to_string().contains("Command+,"));
    }

    #[test]
    fn settings_page_is_searchable() {
        let mut settings = AppSettings::default();
        settings.terminal.default_theme_id = "solarized-light".into();
        let workspace = workspace::WorkspaceData::default();

        let logs = Vec::new();
        let items = list_items(
            &workspace,
            &settings,
            &logs,
            ManageMenu::Settings,
            "solarized",
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Default Theme");
    }

    #[test]
    fn initial_shell_logs_summarize_loaded_workspace() {
        let workspace = workspace::WorkspaceData {
            connections: vec![Connection::default()],
            keys: vec![SshKey::default()],
            identities: vec![Identity::default()],
            known_hosts: vec![KnownHostEntry {
                line_number: 1,
                line: "example.com ssh-ed25519 AAAA".into(),
            }],
            ..workspace::WorkspaceData::default()
        };
        let settings = AppSettings::default();

        let logs = initial_shell_logs(&workspace, &settings);

        assert!(logs[0].contains("started"));
        assert!(logs[1].contains("1 connections"));
        assert!(logs[2].contains("1 known hosts"));
    }

    #[test]
    fn record_shell_log_keeps_latest_entries_with_limit() {
        let logs = Rc::new(RefCell::new(Vec::new()));

        for index in 0..(SHELL_LOG_LIMIT + 5) {
            record_shell_log(&logs, format!("event {index}"));
        }

        let logs = logs.borrow();
        assert_eq!(logs.len(), SHELL_LOG_LIMIT);
        assert_eq!(logs.first().map(String::as_str), Some("event 5"));
        assert_eq!(logs.last().map(String::as_str), Some("event 204"));
    }

    #[test]
    fn logs_page_maps_and_filters_runtime_events() {
        let workspace = workspace::WorkspaceData::default();
        let settings = AppSettings::default();
        let logs = vec![
            "Timon Slint shell started".to_string(),
            "Opening connection #42".to_string(),
        ];

        let items = list_items(&workspace, &settings, &logs, ManageMenu::Logs, "opening");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].badge.to_string(), "LOG");
        assert!(items[0].subtitle.to_string().contains("#42"));
    }

    #[test]
    fn selected_connection_details_uses_requested_connection() {
        let connections = vec![
            Connection {
                id: 1,
                name: "Local".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
            Connection {
                id: 2,
                name: "Remote".into(),
                display_username: "admin".into(),
                host: "10.10.1.110".into(),
                port: 2022,
                connection_type: ConnectionType::Ssh,
                ..Connection::default()
            },
        ];

        assert_eq!(
            selected_connection_details(&connections, "2"),
            SelectedConnectionDetails {
                id: "2".into(),
                name: "Remote".into(),
                endpoint: "admin@10.10.1.110:2022".into(),
                connection_type: "SSH".into(),
            }
        );
    }

    #[test]
    fn selected_connection_details_falls_back_to_first_connection() {
        let connections = vec![Connection {
            id: 7,
            name: "Fallback".into(),
            connection_type: ConnectionType::Serial,
            serial_port: "/dev/tty.usbserial".into(),
            baud_rate: 9600,
            ..Connection::default()
        }];

        assert_eq!(
            selected_connection_details(&connections, "missing"),
            SelectedConnectionDetails {
                id: "7".into(),
                name: "Fallback".into(),
                endpoint: "/dev/tty.usbserial / 9600".into(),
                connection_type: "SERIAL".into(),
            }
        );
    }
}
