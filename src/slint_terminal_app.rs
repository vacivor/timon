#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::env;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::models::{self, Connection, ConnectionType};
use crate::persistence::{
    AppPaths, Database, TerminalColors, TerminalSettings, TerminalThemeEntry,
    builtin_terminal_theme_by_id, load_custom_terminal_themes, load_settings,
};
use slint::winit_030::winit;
use crate::session::{
    ConnectionTarget, SessionCommand, SessionEvent, SessionHandle, connect_target,
};
use crate::slint_terminal;
use crate::slint_terminal_core::{
    TerminalCell, TerminalColor, TerminalEvent, TerminalFont, TerminalKeyModifiers, TerminalPoint,
    TerminalSelection, TerminalSnapshot, TerminalTheme, TerminalUnderlineStyle, TerminalView,
    normalize_selection, selection_contents,
};
use crate::workspace;
#[cfg(test)]
use crate::{persistence, slint_terminal_core};
use alacritty_terminal::vte::ansi::CursorShape;
use copypasta::{ClipboardContext, ClipboardProvider};
use slint::{ComponentHandle, Timer, TimerMode};

const SLINT_TERMINAL_COLS: u16 = 96;
const SLINT_TERMINAL_ROWS: u16 = 32;
const SLINT_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const SLINT_MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);
const SLINT_CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(600);
const DEFAULT_SLINT_WINDOW_TITLE: &str = "Timon Slint Terminal";

struct LiveTerminal {
    terminal: TerminalView,
    theme: TerminalTheme,
    font: TerminalFont,
    pending_session_events: Arc<Mutex<VecDeque<SessionEvent>>>,
    session: SessionHandle,
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
    last_cursor_blink_key: Option<SlintCursorBlinkKey>,
    default_window_title: String,
    window_title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlintCursorBlinkKey {
    line: usize,
    column: usize,
    width: usize,
    shape: CursorShape,
    show_cursor: bool,
    blinking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SlintCursorOverlay {
    visible: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: TerminalColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalGridSize {
    cols: usize,
    rows: usize,
}

impl LiveTerminal {
    fn new(
        terminal: TerminalView,
        theme: TerminalTheme,
        font: TerminalFont,
        pending_session_events: Arc<Mutex<VecDeque<SessionEvent>>>,
        session: SessionHandle,
        default_window_title: String,
    ) -> Self {
        let pixel_width = (SLINT_TERMINAL_COLS as f32 * font.metrics.cell_width).ceil() as u32;
        let pixel_height = (SLINT_TERMINAL_ROWS as f32 * font.metrics.cell_height).ceil() as u32;

        Self {
            terminal,
            theme,
            font,
            pending_session_events,
            session,
            cols: SLINT_TERMINAL_COLS as usize,
            rows: SLINT_TERMINAL_ROWS as usize,
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
                    self.window_title = terminal_window_title(&title, &self.default_window_title);
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
        let key = SlintCursorBlinkKey::from_snapshot(&snapshot);
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

    fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.cursor_blink_started_at = Instant::now();
    }

    fn sync_font_metrics(
        &mut self,
        native_cell_width: f32,
        native_cell_height: f32,
        line_height: f32,
    ) -> bool {
        self.font
            .apply_native_metrics(native_cell_width, native_cell_height, line_height)
    }

    fn sync_window_size(&mut self, pixel_width: u32, pixel_height: u32, scale_factor: f32) -> bool {
        let Some(grid) = terminal_grid_size(pixel_width, pixel_height, scale_factor, &self.font)
        else {
            return false;
        };

        let dimensions_changed = grid.cols != self.cols || grid.rows != self.rows;
        let pixels_changed = pixel_width != self.pixel_width
            || pixel_height != self.pixel_height
            || (scale_factor - self.scale_factor).abs() > f32::EPSILON;

        if !dimensions_changed && !pixels_changed {
            return false;
        }

        self.cols = grid.cols;
        self.rows = grid.rows;
        self.pixel_width = pixel_width;
        self.pixel_height = pixel_height;
        self.scale_factor = scale_factor.max(1.0);

        if dimensions_changed {
            self.terminal.resize(grid.cols, grid.rows);
            let _ = self.session.command_tx.send(SessionCommand::Resize {
                cols: grid.cols.min(u16::MAX as usize) as u16,
                rows: grid.rows.min(u16::MAX as usize) as u16,
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
        let repeated_click = self.last_click_point == Some(point)
            && self
                .last_click_at
                .is_some_and(|last| now.duration_since(last) <= SLINT_MULTI_CLICK_INTERVAL);
        self.click_count = if repeated_click {
            self.click_count.saturating_add(1)
        } else {
            1
        };
        self.last_click_at = Some(now);
        self.last_click_point = Some(point);

        if self.click_count == 2 {
            self.dragging_selection = false;
            self.selection_anchor = None;
            self.selection = Some(self.terminal.word_selection_at_point(&self.theme, point));
            return true;
        }

        if self.click_count >= 3 {
            self.dragging_selection = false;
            self.selection_anchor = None;
            self.selection = Some(self.terminal.token_selection_at_point(&self.theme, point));
            return true;
        }

        self.selection_anchor = Some(point);
        self.selection = Some(normalize_selection(point, point));
        self.dragging_selection = true;
        true
    }

    fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        if self.terminal_mouse_button_down {
            let point = self.terminal.clamped_point_for_logical_position(
                x,
                y,
                self.font.metrics.cell_width,
                self.font.metrics.cell_height,
            );
            self.terminal.handle_mouse_drag(point);
            return false;
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
        let anchor = self.selection_anchor.unwrap_or(point);
        let next = normalize_selection(anchor, point);

        if self
            .selection
            .as_ref()
            .is_some_and(|selection| selection.start == next.start && selection.end == next.end)
        {
            return false;
        }

        self.selection = Some(next);
        true
    }

    fn pointer_up(&mut self, x: f32, y: f32) {
        if self.terminal_mouse_button_down {
            let point = self.terminal.clamped_point_for_logical_position(
                x,
                y,
                self.font.metrics.cell_width,
                self.font.metrics.cell_height,
            );
            self.terminal.handle_mouse_release(point);
            self.terminal_mouse_button_down = false;
        }

        self.dragging_selection = false;
    }

    fn focus_changed(&mut self, focused: bool) -> bool {
        self.terminal.handle_focus_change(focused);
        self.focused = focused;
        self.reset_cursor_blink();
        true
    }

    fn selected_text(&self) -> Option<String> {
        let snapshot = self.terminal.snapshot(&self.theme);
        selection_contents(&snapshot, self.selection.as_ref())
    }

    fn clear_selection(&mut self) -> bool {
        self.selection_anchor = None;
        self.dragging_selection = false;
        self.selection.take().is_some()
    }

    fn paste_text(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        self.reset_cursor_blink();
        self.terminal.scroll_to_bottom();
        let dirty = self.clear_selection();
        let _ = self
            .session
            .command_tx
            .send(SessionCommand::Input(self.terminal.encode_text_input(text)));
        dirty
    }

    fn send_terminal_input(&mut self, payload: Vec<u8>) -> bool {
        self.reset_cursor_blink();
        self.terminal.scroll_to_bottom();
        let dirty = self.clear_selection();
        let _ = self.session.command_tx.send(SessionCommand::Input(payload));
        dirty
    }

    fn disconnect(&self, reason: &str) -> bool {
        self.session
            .command_tx
            .send(SessionCommand::Disconnect(reason.into()))
            .is_ok()
    }
}

impl SlintCursorBlinkKey {
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
    run_with_args(env::args().skip(1))
}

pub fn run_with_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<()> {
    configure_window_backend()?;
    let paths = AppPaths::discover()?;
    let database = Database::new(&paths.database)?;
    let settings = load_settings(&paths.settings).unwrap_or_default();
    let workspace = workspace::WorkspaceData::load(&paths, &database);

    let mut terminal_settings = settings.terminal.clone();
    let terminal_themes = load_custom_terminal_themes(&paths.themes);
    let selection = slint_connection_selection(
        &workspace,
        requested_connection_id(args),
        &terminal_settings.default_theme_id,
    );
    terminal_settings.colors = slint_terminal_colors(
        &terminal_settings,
        &terminal_themes,
        &selection.connection.theme_id,
    );
    let terminal_line_height = terminal_settings.font.line_height;
    let terminal_theme = TerminalTheme::from_settings(&terminal_settings.colors);
    let terminal_font = TerminalFont::from_settings(&terminal_settings.font);
    let mut terminal = TerminalView::new(
        SLINT_TERMINAL_COLS as usize,
        SLINT_TERMINAL_ROWS as usize,
        &terminal_settings,
    );
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let pending_session_events = Arc::new(Mutex::new(VecDeque::new()));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let session = runtime
        .block_on(connect_target(
            ConnectionTarget {
                connection: selection.connection,
                key: selection.key,
                identity: selection.identity,
                known_hosts_path: paths.known_hosts.clone(),
                cols: SLINT_TERMINAL_COLS,
                rows: SLINT_TERMINAL_ROWS,
            },
            event_tx,
        ))
        .map_err(anyhow::Error::msg)?;
    terminal.set_outbound(session.command_tx.clone());

    let ui = slint_terminal::TerminalWindow::new()?;
    let state = Rc::new(RefCell::new(LiveTerminal::new(
        terminal,
        terminal_theme,
        terminal_font,
        Arc::clone(&pending_session_events),
        session,
        selection.fallback_title,
    )));

    {
        let mut state = state.borrow_mut();
        ui.set_terminal_font_family(state.font.family_name.clone().into());
        ui.set_terminal_font_size(state.font.size);
        sync_terminal_layout(&ui, &mut state, terminal_line_height);
        sync_terminal_render(&ui, &mut state);
    }
    let state_for_input = Rc::clone(&state);
    let ui_weak_for_input = ui.as_weak();
    ui.on_input(move |text, alt, control, shift, meta| {
        let mut state = state_for_input.borrow_mut();
        let modifiers = TerminalKeyModifiers {
            alt,
            control,
            shift,
            meta,
        };
        if is_copy_shortcut(&text, modifiers) {
            if let Some(contents) = state.selected_text() {
                write_clipboard(contents);
            }
            return;
        }
        if is_paste_shortcut(&text, modifiers) {
            if let Some(contents) = read_clipboard() {
                if state.paste_text(&contents) {
                    if let Some(ui) = ui_weak_for_input.upgrade() {
                        sync_terminal_render(&ui, &mut state);
                    }
                }
            }
            return;
        }
        let Some(payload) = state.terminal.encode_key_text(&text, modifiers) else {
            return;
        };
        if state.send_terminal_input(payload) {
            if let Some(ui) = ui_weak_for_input.upgrade() {
                sync_terminal_render(&ui, &mut state);
            }
        }
    });
    let state_for_pointer_down = Rc::clone(&state);
    let ui_weak_for_pointer_down = ui.as_weak();
    ui.on_pointer_down(move |x, y| {
        let mut state = state_for_pointer_down.borrow_mut();
        if state.pointer_down(x, y) {
            if let Some(ui) = ui_weak_for_pointer_down.upgrade() {
                sync_terminal_render(&ui, &mut state);
            }
        }
    });
    let state_for_pointer_moved = Rc::clone(&state);
    let ui_weak_for_pointer_moved = ui.as_weak();
    ui.on_pointer_moved(move |x, y| {
        let mut state = state_for_pointer_moved.borrow_mut();
        if state.pointer_moved(x, y) {
            if let Some(ui) = ui_weak_for_pointer_moved.upgrade() {
                sync_terminal_render(&ui, &mut state);
            }
        }
    });
    let state_for_pointer_up = Rc::clone(&state);
    ui.on_pointer_up(move |x, y| {
        state_for_pointer_up.borrow_mut().pointer_up(x, y);
    });
    let state_for_focus_changed = Rc::clone(&state);
    let ui_weak_for_focus_changed = ui.as_weak();
    ui.on_focus_changed(move |focused| {
        let mut state = state_for_focus_changed.borrow_mut();
        if state.focus_changed(focused) {
            if let Some(ui) = ui_weak_for_focus_changed.upgrade() {
                sync_terminal_render(&ui, &mut state);
            }
        }
    });
    let state_for_scroll = Rc::clone(&state);
    let ui_weak_for_scroll = ui.as_weak();
    ui.on_scroll(move |delta_y, x, y| {
        let mut state = state_for_scroll.borrow_mut();
        if state.scroll(delta_y, x, y) {
            if let Some(ui) = ui_weak_for_scroll.upgrade() {
                sync_terminal_render(&ui, &mut state);
            }
        }
    });
    let state_for_session_event = Rc::clone(&state);
    let ui_weak_for_session_event = ui.as_weak();
    ui.on_session_event_ready(move || {
        let mut state = state_for_session_event.borrow_mut();
        let dirty = state.drain_session_events();
        if let Some(ui) = ui_weak_for_session_event.upgrade() {
            sync_terminal_runtime_state(&ui, &mut state, dirty, Instant::now());
        }
    });
    spawn_session_event_forwarder(&runtime, event_rx, pending_session_events, ui.as_weak());

    let timer = Timer::default();
    let ui_weak = ui.as_weak();
    let state_for_timer = Rc::clone(&state);
    timer.start(TimerMode::Repeated, SLINT_FRAME_INTERVAL, move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        let mut state = state_for_timer.borrow_mut();
        let now = Instant::now();
        let dirty = sync_terminal_layout(&ui, &mut state, terminal_line_height);
        sync_terminal_runtime_state(&ui, &mut state, dirty, now);
    });

    let run_result = ui.run();

    {
        let state = state.borrow();
        state.disconnect("窗口关闭");
    }
    runtime.block_on(async {
        tokio::time::sleep(Duration::from_millis(50)).await;
    });
    runtime.shutdown_timeout(Duration::from_millis(250));

    run_result.map_err(Into::into)
}

fn spawn_session_event_forwarder(
    runtime: &tokio::runtime::Runtime,
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<SessionEvent>,
    pending_session_events: Arc<Mutex<VecDeque<SessionEvent>>>,
    ui_weak: slint::Weak<slint_terminal::TerminalWindow>,
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

fn sync_terminal_render(ui: &slint_terminal::TerminalWindow, state: &mut LiveTerminal) {
    let snapshot = state.terminal.snapshot(&state.theme);
    let cursor_visible = state.focused && state.cursor_visible;
    let overlay = cursor_overlay_from_snapshot(&snapshot, &state.font, cursor_visible);

    ui.set_window_title(state.window_title.clone().into());
    ui.set_terminal_background(slint_color(state.theme.background.rgba8()));
    ui.set_cells(snapshot_to_cells(
        &snapshot,
        state.selection.as_ref(),
        &state.font,
        cursor_visible,
    ));
    ui.set_decorations(snapshot_to_decorations(&snapshot, &state.font));
    ui.set_cursor_overlay_visible(overlay.visible);
    ui.set_cursor_overlay_x(overlay.x);
    ui.set_cursor_overlay_y(overlay.y);
    ui.set_cursor_overlay_width(overlay.width);
    ui.set_cursor_overlay_height(overlay.height);
    ui.set_cursor_overlay_color(slint_color(overlay.color.rgba8()));
}

fn sync_terminal_runtime_state(
    ui: &slint_terminal::TerminalWindow,
    state: &mut LiveTerminal,
    mut dirty: bool,
    now: Instant,
) -> bool {
    dirty |= state.drain_terminal_events();
    dirty |= state.update_cursor_blink(now);
    if dirty {
        sync_terminal_render(ui, state);
    }
    dirty
}

fn sync_terminal_layout(
    ui: &slint_terminal::TerminalWindow,
    state: &mut LiveTerminal,
    line_height: f32,
) -> bool {
    let window = ui.window();
    let mut dirty = sync_terminal_font_metrics(ui, state, line_height);
    dirty |= state.sync_window_size(
        window.size().width,
        window.size().height,
        window.scale_factor(),
    );
    dirty
}

fn sync_terminal_font_metrics(
    ui: &slint_terminal::TerminalWindow,
    state: &mut LiveTerminal,
    line_height: f32,
) -> bool {
    state.sync_font_metrics(
        ui.get_terminal_native_cell_width(),
        ui.get_terminal_native_cell_height(),
        line_height,
    )
}

fn snapshot_to_cells(
    snapshot: &TerminalSnapshot,
    selection: Option<&TerminalSelection>,
    font: &TerminalFont,
    cursor_visible: bool,
) -> slint::ModelRc<slint_terminal::TerminalCellItem> {
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

            slint_terminal::TerminalCellItem {
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

fn snapshot_to_decorations(
    snapshot: &TerminalSnapshot,
    font: &TerminalFont,
) -> slint::ModelRc<slint_terminal::TerminalDecorationItem> {
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
            push_underline_decorations(
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
            push_decoration(
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

fn push_underline_decorations(
    decorations: &mut Vec<slint_terminal::TerminalDecorationItem>,
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
            push_decoration(decorations, x, baseline_y, width, stroke, color);
        }
        TerminalUnderlineStyle::Double => {
            push_decoration(decorations, x, baseline_y, width, stroke, color);
            push_decoration(
                decorations,
                x,
                (baseline_y - stroke * 2.0).max(y),
                width,
                stroke,
                color,
            );
        }
        TerminalUnderlineStyle::Dotted => {
            push_segmented_decoration(decorations, x, baseline_y, width, stroke, stroke, color);
        }
        TerminalUnderlineStyle::Dashed => {
            push_segmented_decoration(
                decorations,
                x,
                baseline_y,
                width,
                (stroke * 3.0).max(2.0),
                stroke,
                color,
            );
        }
        TerminalUnderlineStyle::Curly => {
            push_curly_decoration(decorations, x, baseline_y, width, stroke, color);
        }
    }
}

fn push_segmented_decoration(
    decorations: &mut Vec<slint_terminal::TerminalDecorationItem>,
    x: f32,
    y: f32,
    width: f32,
    segment_width: f32,
    stroke: f32,
    color: TerminalColor,
) {
    let gap = stroke.max(1.0);
    let mut current_x = x;
    let end_x = x + width;

    while current_x < end_x {
        let next_width = segment_width.min(end_x - current_x);
        push_decoration(decorations, current_x, y, next_width, stroke, color);
        current_x += segment_width + gap;
    }
}

fn push_curly_decoration(
    decorations: &mut Vec<slint_terminal::TerminalDecorationItem>,
    x: f32,
    y: f32,
    width: f32,
    stroke: f32,
    color: TerminalColor,
) {
    let segment_width = (stroke * 2.0).max(2.0);
    let gap = stroke.max(1.0);
    let mut current_x = x;
    let mut high = false;
    let end_x = x + width;

    while current_x < end_x {
        let next_width = segment_width.min(end_x - current_x);
        let offset = if high { -stroke } else { 0.0 };
        push_decoration(
            decorations,
            current_x,
            y + offset,
            next_width,
            stroke,
            color,
        );
        current_x += segment_width + gap;
        high = !high;
    }
}

fn push_decoration(
    decorations: &mut Vec<slint_terminal::TerminalDecorationItem>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: TerminalColor,
) {
    decorations.push(slint_terminal::TerminalDecorationItem {
        x,
        y,
        width,
        height,
        color: slint_color(color.rgba8()),
    });
}

fn decoration_stroke_width(font: &TerminalFont) -> f32 {
    (font.metrics.cell_height * 0.08).round().clamp(1.0, 2.0)
}

fn cursor_overlay_from_snapshot(
    snapshot: &TerminalSnapshot,
    font: &TerminalFont,
    cursor_visible: bool,
) -> SlintCursorOverlay {
    let hidden = SlintCursorOverlay {
        visible: false,
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        color: snapshot.cursor_color,
    };

    if !cursor_visible
        || !snapshot.show_cursor
        || matches!(
            snapshot.cursor_shape,
            CursorShape::Block | CursorShape::HollowBlock | CursorShape::Hidden
        )
    {
        return hidden;
    }

    let cell_width = font.metrics.cell_width.max(1.0);
    let cell_height = font.metrics.cell_height.max(1.0);
    let stroke = cursor_overlay_stroke_width(font);
    let x = snapshot.cursor_column as f32 * cell_width;
    let y = snapshot.cursor_line as f32 * cell_height;

    match snapshot.cursor_shape {
        CursorShape::Beam => SlintCursorOverlay {
            visible: true,
            x,
            y,
            width: stroke,
            height: cell_height,
            color: snapshot.cursor_color,
        },
        CursorShape::Underline => SlintCursorOverlay {
            visible: true,
            x,
            y: y + cell_height - stroke,
            width: snapshot.cursor_width.max(1) as f32 * cell_width,
            height: stroke,
            color: snapshot.cursor_color,
        },
        CursorShape::Block | CursorShape::HollowBlock | CursorShape::Hidden => hidden,
    }
}

fn cursor_overlay_stroke_width(font: &TerminalFont) -> f32 {
    (font.metrics.cell_width * 0.15).round().clamp(1.0, 2.0)
}

fn terminal_grid_size(
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
    font: &TerminalFont,
) -> Option<TerminalGridSize> {
    if pixel_width == 0 || pixel_height == 0 {
        return None;
    }

    let scale_factor = scale_factor.max(1.0);
    let logical_width = pixel_width as f32 / scale_factor;
    let logical_height = pixel_height as f32 / scale_factor;
    let cols = (logical_width / font.metrics.cell_width.max(1.0))
        .floor()
        .max(2.0) as usize;
    let rows = (logical_height / font.metrics.cell_height.max(1.0))
        .floor()
        .max(2.0) as usize;

    Some(TerminalGridSize { cols, rows })
}

fn cursor_visible_for_elapsed(elapsed: Duration) -> bool {
    let interval = SLINT_CURSOR_BLINK_INTERVAL.as_millis().max(1);
    (elapsed.as_millis() / interval) % 2 == 0
}

fn terminal_window_title(title: &str, fallback: &str) -> String {
    let normalized = title
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let title = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    if title.is_empty() {
        fallback.into()
    } else {
        title.chars().take(256).collect()
    }
}

fn slint_color(color: impl Into<[u8; 4]>) -> slint::Color {
    let [red, green, blue, alpha] = color.into();
    slint::Color::from_argb_u8(alpha, red, green, blue)
}

fn slint_selection_contains(selection: Option<&TerminalSelection>, cell: &TerminalCell) -> bool {
    let Some(selection) = selection else {
        return false;
    };

    let cell_start = (cell.line, cell.column);
    let cell_end = (cell.line, cell.column + cell.width.saturating_sub(1));
    let selection_start = (selection.start.line, selection.start.column);
    let selection_end = (selection.end.line, selection.end.column);

    cell_end >= selection_start && cell_start <= selection_end
}

fn slint_cursor_covers_cell(snapshot: &TerminalSnapshot, cell: &TerminalCell) -> bool {
    cell.line == snapshot.cursor_line
        && cell.column < snapshot.cursor_column + snapshot.cursor_width.max(1)
        && cell.column + cell.width.max(1) > snapshot.cursor_column
}

fn is_copy_shortcut(text: &str, modifiers: TerminalKeyModifiers) -> bool {
    shortcut_matches(text, modifiers, 'c')
}

fn is_paste_shortcut(text: &str, modifiers: TerminalKeyModifiers) -> bool {
    shortcut_matches(text, modifiers, 'v')
}

fn shortcut_matches(text: &str, modifiers: TerminalKeyModifiers, key: char) -> bool {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    if chars.next().is_some() || !ch.eq_ignore_ascii_case(&key) {
        return false;
    }

    if cfg!(target_os = "macos") {
        modifiers.control && !modifiers.meta
    } else {
        (modifiers.control && modifiers.shift) || modifiers.meta
    }
}

fn write_clipboard(contents: String) {
    if let Ok(mut clipboard) = ClipboardContext::new() {
        let _ = clipboard.set_contents(contents);
    }
}

fn read_clipboard() -> Option<String> {
    ClipboardContext::new()
        .ok()
        .and_then(|mut clipboard| clipboard.get_contents().ok())
}

fn slint_terminal_colors(
    settings: &TerminalSettings,
    terminal_themes: &[TerminalThemeEntry],
    theme_id: &str,
) -> TerminalColors {
    match theme_id {
        "default" => settings.colors.clone(),
        other if other == settings.default_theme_id => settings.colors.clone(),
        other => terminal_themes
            .iter()
            .find(|theme| theme.id == other)
            .or_else(|| builtin_terminal_theme_by_id(other))
            .map(|theme| theme.colors.clone())
            .unwrap_or_else(|| {
                terminal_themes
                    .iter()
                    .find(|theme| theme.id == "atom-one-light")
                    .or_else(|| builtin_terminal_theme_by_id("atom-one-light"))
                    .map(|theme| theme.colors.clone())
                    .unwrap_or_else(TerminalColors::atom_one_light)
            }),
    }
}

struct SlintConnectionSelection {
    connection: Connection,
    key: Option<models::Key>,
    identity: Option<models::Identity>,
    fallback_title: String,
}

fn requested_connection_id(args: impl IntoIterator<Item = String>) -> Option<i64> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--connection-id" {
            return args.next().and_then(|value| value.parse::<i64>().ok());
        }

        if let Some(value) = arg.strip_prefix("--connection-id=") {
            return value.parse::<i64>().ok();
        }
    }

    None
}

fn slint_connection_selection(
    workspace: &workspace::WorkspaceData,
    requested_connection_id: Option<i64>,
    default_theme_id: &str,
) -> SlintConnectionSelection {
    let connection = requested_connection_id
        .and_then(|id| {
            workspace
                .connections
                .iter()
                .find(|connection| connection.id == id)
        })
        .or_else(|| {
            workspace
                .connections
                .iter()
                .find(|connection| connection.connection_type == ConnectionType::Local)
        })
        .cloned()
        .unwrap_or_else(|| {
            let mut connection = Connection::default();
            connection.name = "Local Shell".into();
            connection.connection_type = ConnectionType::Local;
            connection.theme_id = default_theme_id.into();
            connection
        });

    let key = connection
        .effective_key_id
        .and_then(|id| workspace.keys.iter().find(|key| key.id == id))
        .cloned();
    let identity = connection
        .identity_id
        .and_then(|id| {
            workspace
                .identities
                .iter()
                .find(|identity| identity.id == id)
        })
        .cloned();
    let fallback_title = terminal_fallback_title(&connection);

    SlintConnectionSelection {
        connection,
        key,
        identity,
        fallback_title,
    }
}

fn terminal_fallback_title(connection: &Connection) -> String {
    match connection.connection_type {
        ConnectionType::Local => {
            if connection.name.trim().is_empty() {
                "Local Shell".into()
            } else {
                connection.name.clone()
            }
        }
        ConnectionType::Ssh | ConnectionType::Serial => {
            if connection.name.trim().is_empty() {
                DEFAULT_SLINT_WINDOW_TITLE.into()
            } else {
                connection.name.clone()
            }
        }
    }
}

fn slint_local_connection(
    workspace: &workspace::WorkspaceData,
    default_theme_id: &str,
) -> Connection {
    slint_connection_selection(workspace, None, default_theme_id).connection
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;

    #[test]
    fn cursor_blink_visibility_alternates_by_interval() {
        assert!(cursor_visible_for_elapsed(Duration::from_millis(0)));
        assert!(cursor_visible_for_elapsed(Duration::from_millis(599)));
        assert!(!cursor_visible_for_elapsed(Duration::from_millis(600)));
        assert!(!cursor_visible_for_elapsed(Duration::from_millis(1199)));
        assert!(cursor_visible_for_elapsed(Duration::from_millis(1200)));
    }

    #[test]
    fn terminal_window_title_sanitizes_control_characters() {
        assert_eq!(
            terminal_window_title("vim\tproject\n\u{1b}]0;bad", "Fallback"),
            "vim project ]0;bad"
        );
    }

    #[test]
    fn terminal_window_title_uses_fallback_for_empty_titles() {
        assert_eq!(terminal_window_title("\n\t", "Fallback"), "Fallback");
    }

    #[test]
    fn requested_connection_id_accepts_space_and_equals_forms() {
        assert_eq!(
            requested_connection_id(vec!["--connection-id".into(), "42".into()]),
            Some(42)
        );
        assert_eq!(
            requested_connection_id(vec!["--connection-id=7".into()]),
            Some(7)
        );
        assert_eq!(
            requested_connection_id(vec!["--connection-id".into(), "bad".into()]),
            None
        );
    }

    #[test]
    fn slint_terminal_colors_uses_settings_for_default_theme_aliases() {
        let mut settings = TerminalSettings::default();
        settings.default_theme_id = "custom-default".into();
        settings.colors.primary.background = "#123456".into();

        assert_eq!(
            slint_terminal_colors(&settings, &[], "default")
                .primary
                .background,
            "#123456"
        );
        assert_eq!(
            slint_terminal_colors(&settings, &[], "custom-default")
                .primary
                .background,
            "#123456"
        );
    }

    #[test]
    fn slint_terminal_colors_resolves_builtin_theme() {
        let settings = TerminalSettings::default();

        assert_eq!(
            slint_terminal_colors(&settings, &[], "atom-one-dark")
                .primary
                .background,
            TerminalColors::atom_one_dark().primary.background
        );
    }

    #[test]
    fn slint_terminal_colors_prefers_custom_theme_over_builtin() {
        let settings = TerminalSettings::default();
        let mut custom_colors = TerminalColors::atom_one_light();
        custom_colors.primary.background = "#010203".into();
        let themes = vec![TerminalThemeEntry {
            id: "atom-one-dark".into(),
            path: "custom/atom-one-dark.toml".into(),
            colors: custom_colors,
        }];

        assert_eq!(
            slint_terminal_colors(&settings, &themes, "atom-one-dark")
                .primary
                .background,
            "#010203"
        );
    }

    #[test]
    fn slint_connection_selection_uses_requested_connection_and_credentials() {
        let workspace = workspace::WorkspaceData {
            connections: vec![
                Connection {
                    id: 1,
                    name: "Local".into(),
                    connection_type: ConnectionType::Local,
                    ..Connection::default()
                },
                Connection {
                    id: 2,
                    name: "Remote".into(),
                    effective_key_id: Some(10),
                    identity_id: Some(20),
                    connection_type: ConnectionType::Ssh,
                    ..Connection::default()
                },
            ],
            keys: vec![models::Key {
                id: 10,
                name: "prod".into(),
                ..models::Key::default()
            }],
            identities: vec![models::Identity {
                id: 20,
                name: "deploy".into(),
                ..models::Identity::default()
            }],
            ..workspace::WorkspaceData::default()
        };

        let selection = slint_connection_selection(&workspace, Some(2), "default");

        assert_eq!(selection.connection.id, 2);
        assert_eq!(
            selection.key.as_ref().map(|key| key.name.as_str()),
            Some("prod")
        );
        assert_eq!(
            selection
                .identity
                .as_ref()
                .map(|identity| identity.name.as_str()),
            Some("deploy")
        );
        assert_eq!(selection.fallback_title, "Remote");
    }

    #[test]
    fn slint_connection_selection_falls_back_to_local_connection() {
        let workspace = workspace::WorkspaceData {
            connections: vec![Connection {
                id: 1,
                name: "Local Shell".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            }],
            ..workspace::WorkspaceData::default()
        };

        let selection = slint_connection_selection(&workspace, Some(999), "default");

        assert_eq!(selection.connection.id, 1);
        assert_eq!(selection.fallback_title, "Local Shell");
    }

    #[test]
    fn beam_cursor_uses_vertical_overlay() {
        let snapshot = test_snapshot(CursorShape::Beam, 2, 3, 1);
        let font = test_font();

        assert_eq!(
            cursor_overlay_from_snapshot(&snapshot, &font, true),
            SlintCursorOverlay {
                visible: true,
                x: 30.0,
                y: 40.0,
                width: 2.0,
                height: 20.0,
                color: test_cursor_color(),
            }
        );
    }

    #[test]
    fn underline_cursor_uses_bottom_overlay() {
        let snapshot = test_snapshot(CursorShape::Underline, 2, 3, 2);
        let font = test_font();

        assert_eq!(
            cursor_overlay_from_snapshot(&snapshot, &font, true),
            SlintCursorOverlay {
                visible: true,
                x: 30.0,
                y: 58.0,
                width: 20.0,
                height: 2.0,
                color: test_cursor_color(),
            }
        );
    }

    #[test]
    fn block_cursor_does_not_use_overlay() {
        let snapshot = test_snapshot(CursorShape::Block, 2, 3, 1);
        let font = test_font();

        assert!(!cursor_overlay_from_snapshot(&snapshot, &font, true).visible);
    }

    #[test]
    fn cursor_overlay_respects_blink_visibility() {
        let snapshot = test_snapshot(CursorShape::Beam, 2, 3, 1);
        let font = test_font();

        assert!(!cursor_overlay_from_snapshot(&snapshot, &font, false).visible);
    }

    #[test]
    fn block_cursor_cell_respects_cursor_visibility() {
        let mut snapshot = test_snapshot(CursorShape::Block, 0, 0, 1);
        let foreground = TerminalColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 255,
        };
        let background = TerminalColor {
            red: 4,
            green: 5,
            blue: 6,
            alpha: 255,
        };
        snapshot.cursor_text = TerminalColor {
            red: 7,
            green: 8,
            blue: 9,
            alpha: 255,
        };
        snapshot.cursor_color = TerminalColor {
            red: 10,
            green: 11,
            blue: 12,
            alpha: 255,
        };
        snapshot.cells = vec![TerminalCell {
            text: "A".into(),
            fg: foreground,
            bg: background,
            underline: None,
            underline_color: foreground,
            width: 1,
            bold: false,
            italic: false,
            strikeout: false,
            dim: false,
            hidden: false,
            line: 0,
            column: 0,
        }];

        let visible = snapshot_to_cells(&snapshot, None, &test_font(), true);
        let visible_cell = visible.row_data(0).unwrap();
        assert_eq!(
            visible_cell.foreground,
            slint_color(snapshot.cursor_text.rgba8())
        );
        assert_eq!(
            visible_cell.background,
            slint_color(snapshot.cursor_color.rgba8())
        );

        let hidden = snapshot_to_cells(&snapshot, None, &test_font(), false);
        let hidden_cell = hidden.row_data(0).unwrap();
        assert_eq!(hidden_cell.foreground, slint_color(foreground.rgba8()));
        assert_eq!(hidden_cell.background, slint_color(background.rgba8()));
    }

    #[test]
    fn snapshot_to_cells_preserves_wide_cell_geometry() {
        let mut snapshot = test_snapshot(CursorShape::Block, 0, 0, 1);
        snapshot.cells = vec![TerminalCell {
            text: "界".into(),
            fg: test_color(),
            bg: test_cursor_color(),
            underline: None,
            underline_color: test_color(),
            width: 2,
            bold: false,
            italic: false,
            strikeout: false,
            dim: false,
            hidden: false,
            line: 1,
            column: 3,
        }];

        let cells = snapshot_to_cells(&snapshot, None, &test_font(), true);
        let cell = cells.row_data(0).unwrap();

        assert_eq!(cell.x, 30.0);
        assert_eq!(cell.y, 20.0);
        assert_eq!(cell.width, 20.0);
        assert_eq!(cell.height, 20.0);
    }

    #[test]
    fn terminal_input_clears_selection_and_sends_payload() {
        let (mut live, mut command_rx) = test_live_terminal();
        live.selection_anchor = Some(TerminalPoint { line: 0, column: 0 });
        live.selection = Some(TerminalSelection {
            start: TerminalPoint { line: 0, column: 0 },
            end: TerminalPoint { line: 0, column: 2 },
        });

        assert!(live.send_terminal_input(b"a".to_vec()));
        assert!(live.selection.is_none());
        assert!(live.selection_anchor.is_none());

        match command_rx.try_recv() {
            Ok(SessionCommand::Input(bytes)) => assert_eq!(bytes, b"a".to_vec()),
            other => panic!("expected input command, got {other:?}"),
        }
    }

    #[test]
    fn paste_text_clears_selection_and_uses_terminal_encoding() {
        let (mut live, mut command_rx) = test_live_terminal();
        live.selection_anchor = Some(TerminalPoint { line: 0, column: 0 });
        live.selection = Some(TerminalSelection {
            start: TerminalPoint { line: 0, column: 0 },
            end: TerminalPoint { line: 0, column: 2 },
        });

        assert!(live.paste_text("a\nb"));
        assert!(live.selection.is_none());
        assert!(live.selection_anchor.is_none());

        match command_rx.try_recv() {
            Ok(SessionCommand::Input(bytes)) => assert_eq!(bytes, b"a\rb".to_vec()),
            other => panic!("expected paste command, got {other:?}"),
        }
    }

    #[test]
    fn terminal_disconnect_sends_session_disconnect_command() {
        let (live, mut command_rx) = test_live_terminal();

        assert!(live.disconnect("窗口关闭"));

        match command_rx.try_recv() {
            Ok(SessionCommand::Disconnect(reason)) => assert_eq!(reason, "窗口关闭"),
            other => panic!("expected disconnect command, got {other:?}"),
        }
    }

    #[test]
    fn single_underline_decoration_uses_cell_baseline() {
        let mut snapshot = test_snapshot(CursorShape::Block, 0, 0, 1);
        snapshot.cells = vec![test_cell_with_decoration(
            TerminalUnderlineStyle::Single,
            false,
            1,
            2,
            2,
        )];
        let decorations = snapshot_to_decorations(&snapshot, &test_font());

        assert_eq!(decorations.row_count(), 1);
        let decoration = decorations.row_data(0).unwrap();
        assert_eq!(decoration.x, 20.0);
        assert_eq!(decoration.y, 38.0);
        assert_eq!(decoration.width, 20.0);
        assert_eq!(decoration.height, 2.0);
    }

    #[test]
    fn double_underline_decoration_adds_two_lines() {
        let mut snapshot = test_snapshot(CursorShape::Block, 0, 0, 1);
        snapshot.cells = vec![test_cell_with_decoration(
            TerminalUnderlineStyle::Double,
            false,
            0,
            0,
            1,
        )];
        let decorations = snapshot_to_decorations(&snapshot, &test_font());

        assert_eq!(decorations.row_count(), 2);
        assert_eq!(decorations.row_data(0).unwrap().y, 18.0);
        assert_eq!(decorations.row_data(1).unwrap().y, 14.0);
    }

    #[test]
    fn dotted_underline_decoration_is_segmented() {
        let mut snapshot = test_snapshot(CursorShape::Block, 0, 0, 1);
        snapshot.cells = vec![test_cell_with_decoration(
            TerminalUnderlineStyle::Dotted,
            false,
            0,
            0,
            1,
        )];
        let decorations = snapshot_to_decorations(&snapshot, &test_font());

        assert!(decorations.row_count() > 1);
        assert_eq!(decorations.row_data(0).unwrap().width, 2.0);
    }

    #[test]
    fn strikeout_decoration_uses_midline() {
        let mut snapshot = test_snapshot(CursorShape::Block, 0, 0, 1);
        snapshot.cells = vec![test_cell_with_decoration(
            TerminalUnderlineStyle::Single,
            true,
            0,
            0,
            1,
        )];
        let decorations = snapshot_to_decorations(&snapshot, &test_font());

        assert_eq!(decorations.row_count(), 2);
        let strikeout = decorations.row_data(1).unwrap();
        assert_eq!(strikeout.y, 11.2);
        assert_eq!(strikeout.width, 10.0);
    }

    #[test]
    fn terminal_grid_size_returns_none_for_zero_pixels() {
        assert_eq!(terminal_grid_size(0, 800, 1.0, &test_font()), None);
        assert_eq!(terminal_grid_size(1000, 0, 1.0, &test_font()), None);
    }

    #[test]
    fn terminal_grid_size_accounts_for_scale_factor() {
        assert_eq!(
            terminal_grid_size(1000, 800, 2.0, &test_font()),
            Some(TerminalGridSize { cols: 50, rows: 20 })
        );
    }

    #[test]
    fn terminal_grid_size_clamps_to_minimum_grid() {
        assert_eq!(
            terminal_grid_size(1, 1, 1.0, &test_font()),
            Some(TerminalGridSize { cols: 2, rows: 2 })
        );
    }

    #[test]
    fn sync_window_size_sends_resize_only_when_grid_changes() {
        let (mut live, mut command_rx) = test_live_terminal();

        assert!(live.sync_window_size(1000, 800, 1.0));
        assert_eq!(live.cols, 100);
        assert_eq!(live.rows, 40);
        match command_rx.try_recv().unwrap() {
            SessionCommand::Resize { cols, rows } => {
                assert_eq!(cols, 100);
                assert_eq!(rows, 40);
            }
            command => panic!("expected resize command, got {command:?}"),
        }

        assert!(!live.sync_window_size(1000, 800, 1.0));
        assert!(command_rx.try_recv().is_err());
    }

    #[test]
    fn sync_window_size_recomputes_after_native_metrics_change() {
        let (mut live, mut command_rx) = test_live_terminal();

        assert!(live.sync_window_size(1000, 800, 1.0));
        let _ = command_rx.try_recv().unwrap();

        assert!(live.sync_font_metrics(20.0, 20.0, 1.0));
        assert!(live.sync_window_size(1000, 800, 1.0));
        assert_eq!(live.cols, 50);
        assert_eq!(live.rows, 40);
        match command_rx.try_recv().unwrap() {
            SessionCommand::Resize { cols, rows } => {
                assert_eq!(cols, 50);
                assert_eq!(rows, 40);
            }
            command => panic!("expected resize command, got {command:?}"),
        }
    }

    fn test_font() -> TerminalFont {
        TerminalFont {
            size: 13.0,
            metrics: slint_terminal_core::TerminalMetrics {
                cell_width: 10.0,
                cell_height: 20.0,
            },
            family_name: "monospace".into(),
        }
    }

    fn test_snapshot(
        cursor_shape: CursorShape,
        cursor_line: usize,
        cursor_column: usize,
        cursor_width: usize,
    ) -> TerminalSnapshot {
        TerminalSnapshot {
            cells: Vec::new(),
            cursor_line,
            cursor_column,
            cursor_width,
            cursor_shape,
            show_cursor: true,
            cursor_blinking: true,
            background: test_color(),
            cursor_color: test_cursor_color(),
            cursor_text: test_color(),
            selection_background: test_color(),
            selection_foreground: test_color(),
        }
    }

    fn test_color() -> TerminalColor {
        TerminalColor {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 255,
        }
    }

    fn test_cursor_color() -> TerminalColor {
        TerminalColor {
            red: 200,
            green: 210,
            blue: 220,
            alpha: 255,
        }
    }

    fn test_live_terminal() -> (
        LiveTerminal,
        tokio::sync::mpsc::UnboundedReceiver<SessionCommand>,
    ) {
        let settings = persistence::TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let font = test_font();
        let terminal = TerminalView::new(
            SLINT_TERMINAL_COLS as usize,
            SLINT_TERMINAL_ROWS as usize,
            &settings,
        );
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let live = LiveTerminal::new(
            terminal,
            theme,
            font,
            Arc::new(Mutex::new(VecDeque::new())),
            SessionHandle { command_tx },
            DEFAULT_SLINT_WINDOW_TITLE.into(),
        );

        (live, command_rx)
    }

    fn test_cell_with_decoration(
        underline: TerminalUnderlineStyle,
        strikeout: bool,
        line: usize,
        column: usize,
        width: usize,
    ) -> TerminalCell {
        TerminalCell {
            text: "x".into(),
            fg: test_color(),
            bg: test_color(),
            underline: Some(underline),
            underline_color: test_cursor_color(),
            width,
            bold: false,
            italic: false,
            strikeout,
            dim: false,
            hidden: false,
            line,
            column,
        }
    }
}
