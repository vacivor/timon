use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::cell::Flags as TermCellFlags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, RenderableContent, Term, TermMode, point_to_viewport};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb};
use tokio::sync::mpsc;

use crate::persistence::{FontSettings, TerminalColors, TerminalSettings};
use crate::session::SessionCommand;

pub struct TerminalView {
    term: Term<TerminalEventProxy>,
    parser: Processor,
    event_proxy: TerminalEventProxy,
    event_rx: mpsc::UnboundedReceiver<TerminalEvent>,
    cols: usize,
    rows: usize,
}

#[derive(Debug, Clone)]
struct TerminalEventProxy {
    outbound: Arc<Mutex<Option<mpsc::UnboundedSender<SessionCommand>>>>,
    events: mpsc::UnboundedSender<TerminalEvent>,
}

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Title(String),
    ResetTitle,
}

#[derive(Debug, Clone)]
pub struct TerminalTheme {
    pub background: TerminalColor,
    pub foreground: TerminalColor,
    pub cursor_color: TerminalColor,
    pub cursor_text: TerminalColor,
    pub selection_background: TerminalColor,
    pub selection_foreground: TerminalColor,
    pub ansi: [TerminalColor; 16],
}

#[derive(Debug, Clone)]
pub struct TerminalFont {
    pub size: f32,
    pub metrics: TerminalMetrics,
    pub family_name: String,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
}

#[derive(Debug, Clone)]
pub struct TerminalCell {
    pub text: String,
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub underline: Option<TerminalUnderlineStyle>,
    pub underline_color: TerminalColor,
    pub width: usize,
    pub bold: bool,
    pub italic: bool,
    pub strikeout: bool,
    pub dim: bool,
    pub hidden: bool,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalUnderlineStyle {
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Debug, Clone)]
pub struct TerminalSnapshot {
    pub cells: Vec<TerminalCell>,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub cursor_width: usize,
    pub cursor_shape: CursorShape,
    pub show_cursor: bool,
    pub cursor_blinking: bool,
    pub background: TerminalColor,
    pub cursor_color: TerminalColor,
    pub cursor_text: TerminalColor,
    pub selection_background: TerminalColor,
    pub selection_foreground: TerminalColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelection {
    pub start: TerminalPoint,
    pub end: TerminalPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalPoint {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalKeyModifiers {
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub meta: bool,
}

impl TerminalView {
    pub fn new(cols: usize, rows: usize, settings: &TerminalSettings) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let event_proxy = TerminalEventProxy {
            outbound: Arc::new(Mutex::new(None)),
            events: event_tx,
        };

        Self {
            term: Term::new(
                config_from_terminal(settings),
                &TermSize::new(cols, rows),
                event_proxy.clone(),
            ),
            parser: Processor::new(),
            event_proxy,
            event_rx,
            cols,
            rows,
        }
    }

    pub fn set_outbound(&mut self, outbound: mpsc::UnboundedSender<SessionCommand>) {
        if let Ok(mut sender) = self.event_proxy.outbound.lock() {
            *sender = Some(outbound);
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub fn push_local_line(&mut self, line: &str) {
        self.feed(line.as_bytes());
        self.feed(b"\r\n");
    }

    pub fn try_recv_event(&mut self) -> Option<TerminalEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn snapshot(&self, theme: &TerminalTheme) -> TerminalSnapshot {
        let renderable = self.term.renderable_content();
        snapshot_from_renderable(
            renderable,
            self.cols,
            self.rows,
            theme,
            self.term.cursor_style().blinking,
        )
    }

    pub fn word_selection_at_point(
        &self,
        theme: &TerminalTheme,
        point: TerminalPoint,
    ) -> TerminalSelection {
        let snapshot = self.snapshot(theme);
        selection_at(&snapshot, point, word_cell_class)
    }

    pub fn token_selection_at_point(
        &self,
        theme: &TerminalTheme,
        point: TerminalPoint,
    ) -> TerminalSelection {
        let snapshot = self.snapshot(theme);
        selection_at(&snapshot, point, token_cell_class)
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols.max(2);
        self.rows = rows.max(2);
        self.term.resize(TermSize::new(self.cols, self.rows));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn handle_scroll(&mut self, delta: i32, point: TerminalPoint) {
        if delta == 0 {
            return;
        }

        let renderable = self.term.renderable_content();
        let mode = renderable.mode;

        if mode.contains(TermMode::ALT_SCREEN) {
            if mode.intersects(TermMode::MOUSE_MODE) {
                self.send_mouse_wheel(delta, point, mode.contains(TermMode::SGR_MOUSE));
            } else if mode.contains(TermMode::ALTERNATE_SCROLL) {
                self.send_alternate_scroll(delta, mode.contains(TermMode::APP_CURSOR));
            }
        } else if mode.intersects(TermMode::MOUSE_MODE) {
            self.send_mouse_wheel(delta, point, mode.contains(TermMode::SGR_MOUSE));
        } else {
            self.term.scroll_display(Scroll::Delta(delta));
        }
    }

    pub fn handle_mouse_press(&mut self, point: TerminalPoint) -> bool {
        let mode = self.term.mode();
        if !mode.intersects(TermMode::MOUSE_MODE) {
            return false;
        }

        self.send_mouse_button(0, point, true, mode.contains(TermMode::SGR_MOUSE));
        true
    }

    pub fn handle_mouse_release(&mut self, point: TerminalPoint) -> bool {
        let mode = self.term.mode();
        if !mode.intersects(TermMode::MOUSE_MODE) {
            return false;
        }

        self.send_mouse_button(0, point, false, mode.contains(TermMode::SGR_MOUSE));
        true
    }

    pub fn handle_mouse_drag(&mut self, point: TerminalPoint) -> bool {
        let mode = self.term.mode();
        if !mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) {
            return false;
        }

        self.send_mouse_button(32, point, true, mode.contains(TermMode::SGR_MOUSE));
        true
    }

    pub fn handle_focus_change(&mut self, focused: bool) -> bool {
        self.term.is_focused = focused;
        if !self.term.mode().contains(TermMode::FOCUS_IN_OUT) {
            return false;
        }

        let payload = if focused { b"\x1b[I" } else { b"\x1b[O" };
        self.event_proxy
            .send_input(SessionCommand::Input(payload.to_vec()));
        true
    }

    pub fn encode_key_text(&self, text: &str, modifiers: TerminalKeyModifiers) -> Option<Vec<u8>> {
        if text.is_empty() {
            return None;
        }

        if modifiers.command_modifier() {
            return None;
        }

        if modifiers.terminal_control() {
            if let Some(sequence) = ctrl_sequence(text) {
                return Some(sequence);
            }
        }

        let app_cursor = self.term.mode().contains(TermMode::APP_CURSOR);
        if let Some(sequence) = special_key_sequence(text, modifiers, app_cursor) {
            return Some(sequence);
        }

        let mut bytes = Vec::new();
        if modifiers.alt {
            bytes.push(0x1b);
        }
        bytes.extend_from_slice(text.as_bytes());
        Some(bytes)
    }

    pub fn encode_text_input(&self, content: &str) -> Vec<u8> {
        let normalized = content.replace("\r\n", "\n").replace('\r', "\n");

        if normalized.contains('\n') && self.term.mode().contains(TermMode::BRACKETED_PASTE) {
            let mut bytes = b"\x1b[200~".to_vec();
            bytes.extend_from_slice(normalized.as_bytes());
            bytes.extend_from_slice(b"\x1b[201~");
            bytes
        } else {
            normalized.replace('\n', "\r").into_bytes()
        }
    }

    pub fn point_for_logical_position(
        &self,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<TerminalPoint> {
        if x < 0.0 || y < 0.0 {
            return None;
        }

        let column = (x / cell_width.max(1.0)).floor() as usize;
        let line = (y / cell_height.max(1.0)).floor() as usize;

        if column >= self.cols || line >= self.rows {
            return None;
        }

        Some(TerminalPoint { line, column })
    }

    pub fn clamped_point_for_logical_position(
        &self,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
    ) -> TerminalPoint {
        let column = (x / cell_width.max(1.0)).floor() as isize;
        let line = (y / cell_height.max(1.0)).floor() as isize;

        TerminalPoint {
            line: line.clamp(0, self.rows.saturating_sub(1) as isize) as usize,
            column: column.clamp(0, self.cols.saturating_sub(1) as isize) as usize,
        }
    }

    fn send_alternate_scroll(&self, delta: i32, app_cursor: bool) {
        let sequence = if delta > 0 {
            cursor_key(app_cursor, b'A', TerminalKeyModifiers::default())
        } else {
            cursor_key(app_cursor, b'B', TerminalKeyModifiers::default())
        };

        for _ in 0..delta.abs() {
            self.event_proxy
                .send_input(SessionCommand::Input(sequence.clone()));
        }
    }

    fn send_mouse_wheel(&self, delta: i32, point: TerminalPoint, sgr: bool) {
        let column = point.column.saturating_add(1) as u16;
        let line = point.line.saturating_add(1) as u16;

        for _ in 0..delta.abs() {
            let button = if delta > 0 { 64 } else { 65 };
            let payload = if sgr {
                format!("\x1b[<{};{};{}M", button, column, line).into_bytes()
            } else {
                vec![
                    0x1b,
                    b'[',
                    b'M',
                    (32 + button) as u8,
                    32 + column.min(223) as u8,
                    32 + line.min(223) as u8,
                ]
            };
            self.event_proxy.send_input(SessionCommand::Input(payload));
        }
    }

    fn send_mouse_button(&self, button: u8, point: TerminalPoint, pressed: bool, sgr: bool) {
        let column = point.column.saturating_add(1) as u16;
        let line = point.line.saturating_add(1) as u16;
        let payload = if sgr {
            let suffix = if pressed { 'M' } else { 'm' };
            format!("\x1b[<{};{};{}{}", button, column, line, suffix).into_bytes()
        } else {
            let button = if pressed { button } else { 3 };
            vec![
                0x1b,
                b'[',
                b'M',
                (32 + button) as u8,
                32 + column.min(223) as u8,
                32 + line.min(223) as u8,
            ]
        };

        self.event_proxy.send_input(SessionCommand::Input(payload));
    }
}

impl TerminalTheme {
    pub fn from_settings(colors: &TerminalColors) -> Self {
        let fallback = TerminalColors::atom_one_light();
        let parse = |value: &str, fallback_value: &str| {
            parse_hex_color(value).unwrap_or_else(|| parse_hex_color(fallback_value).unwrap())
        };
        let normal = colors.normal.as_array();
        let bright = colors.bright.as_array();
        let fallback_normal = fallback.normal.as_array();
        let fallback_bright = fallback.bright.as_array();
        let ansi = std::array::from_fn(|index| {
            if index < 8 {
                parse(&normal[index], &fallback_normal[index])
            } else {
                parse(&bright[index - 8], &fallback_bright[index - 8])
            }
        });

        Self {
            background: parse(&colors.primary.background, &fallback.primary.background),
            foreground: parse(&colors.primary.foreground, &fallback.primary.foreground),
            cursor_color: parse(&colors.cursor.cursor, &fallback.cursor.cursor),
            cursor_text: parse(&colors.cursor.text, &fallback.cursor.text),
            selection_background: parse(
                &colors.selection.background,
                &fallback.selection.background,
            ),
            selection_foreground: parse(&colors.selection.text, &fallback.selection.text),
            ansi,
        }
    }
}

impl TerminalFont {
    pub fn from_settings(settings: &FontSettings) -> Self {
        let size = settings.size.max(1.0);
        let line_height = settings.line_height.max(1.0);
        Self {
            size,
            metrics: TerminalMetrics {
                cell_width: snap_terminal_metric(size * 0.62),
                cell_height: snap_terminal_metric(size * line_height),
            },
            family_name: settings.family.clone(),
        }
    }

    pub fn apply_native_metrics(
        &mut self,
        cell_width: f32,
        native_cell_height: f32,
        line_height: f32,
    ) -> bool {
        if cell_width <= 0.0 || native_cell_height <= 0.0 {
            return false;
        }

        let next_metrics = TerminalMetrics {
            cell_width: snap_terminal_metric(cell_width),
            cell_height: snap_terminal_metric(native_cell_height * line_height.max(1.0)),
        };
        if (self.metrics.cell_width - next_metrics.cell_width).abs() <= f32::EPSILON
            && (self.metrics.cell_height - next_metrics.cell_height).abs() <= f32::EPSILON
        {
            return false;
        }

        self.metrics = next_metrics;
        true
    }
}

fn snap_terminal_metric(value: f32) -> f32 {
    value.round().max(1.0)
}

impl TerminalColor {
    pub fn rgba8(self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }

    fn from_rgb(rgb: Rgb) -> Self {
        Self {
            red: rgb.r,
            green: rgb.g,
            blue: rgb.b,
            alpha: 255,
        }
    }

    fn scale_alpha(self, factor: f32) -> Self {
        Self {
            alpha: ((self.alpha as f32 * factor).round()).clamp(0.0, 255.0) as u8,
            ..self
        }
    }
}

impl TerminalKeyModifiers {
    fn terminal_control(self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.control
        }
    }

    fn command_modifier(self) -> bool {
        if cfg!(target_os = "macos") {
            self.control
        } else {
            self.meta
        }
    }
}

pub fn normalize_selection(anchor: TerminalPoint, head: TerminalPoint) -> TerminalSelection {
    if (head.line, head.column) < (anchor.line, anchor.column) {
        TerminalSelection {
            start: head,
            end: anchor,
        }
    } else {
        TerminalSelection {
            start: anchor,
            end: head,
        }
    }
}

pub fn selection_contents(
    snapshot: &TerminalSnapshot,
    selection: Option<&TerminalSelection>,
) -> Option<String> {
    let selection = selection?;
    let mut rows = std::collections::BTreeMap::<usize, Vec<_>>::new();

    for cell in &snapshot.cells {
        if !selection_contains(selection, cell) {
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

                if !cell.hidden {
                    line_output.push_str(&cell.text);
                }
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

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(payload) => {
                self.send_input(SessionCommand::Input(payload.into_bytes()));
            }
            Event::Title(title) => {
                let _ = self.events.send(TerminalEvent::Title(title));
            }
            Event::ResetTitle => {
                let _ = self.events.send(TerminalEvent::ResetTitle);
            }
            _ => {}
        }
    }
}

impl TerminalEventProxy {
    fn send_input(&self, command: SessionCommand) {
        if let Ok(sender) = self.outbound.lock() {
            if let Some(outbound) = &*sender {
                let _ = outbound.send(command);
            }
        }
    }
}

fn snapshot_from_renderable(
    renderable: RenderableContent<'_>,
    cols: usize,
    rows: usize,
    theme: &TerminalTheme,
    cursor_blinking: bool,
) -> TerminalSnapshot {
    let mut cells = Vec::with_capacity(cols * rows);

    for indexed in renderable.display_iter {
        let Some(viewport_point) = point_to_viewport(renderable.display_offset, indexed.point)
        else {
            continue;
        };
        let line = viewport_point.line;
        let column = viewport_point.column.0;

        if line >= rows || column >= cols {
            continue;
        }

        let flags = indexed.cell.flags;
        if flags
            .intersects(TermCellFlags::WIDE_CHAR_SPACER | TermCellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }

        let mut text = indexed.cell.c.to_string();
        if let Some(zerowidth) = indexed.cell.zerowidth() {
            text.extend(zerowidth.iter().copied());
        }

        let (fg, bg) = cell_colors(
            indexed.cell.fg,
            indexed.cell.bg,
            flags,
            renderable.colors,
            theme,
        );
        let underline_color = indexed
            .cell
            .underline_color()
            .map(|color| resolve_color(color, renderable.colors, theme))
            .unwrap_or(fg);

        cells.push(TerminalCell {
            text,
            fg,
            bg,
            underline: underline_style(flags),
            underline_color,
            width: if flags.contains(TermCellFlags::WIDE_CHAR) {
                2
            } else {
                1
            },
            bold: flags.intersects(TermCellFlags::BOLD | TermCellFlags::DIM_BOLD),
            italic: flags.contains(TermCellFlags::ITALIC),
            strikeout: flags.contains(TermCellFlags::STRIKEOUT),
            dim: flags.contains(TermCellFlags::DIM),
            hidden: flags.contains(TermCellFlags::HIDDEN),
            line,
            column,
        });
    }

    let cursor_line = renderable.cursor.point.line.0.max(0) as usize;
    let cursor_column = renderable.cursor.point.column.0;
    let show_cursor =
        renderable.display_offset == 0 && renderable.cursor.shape != CursorShape::Hidden;
    let cursor_width = cells
        .iter()
        .find(|cell| {
            cell.line == cursor_line
                && cell.column <= cursor_column
                && cursor_column < cell.column + cell.width.max(1)
        })
        .map(|cell| cell.width.max(1))
        .unwrap_or(1);

    TerminalSnapshot {
        cells,
        cursor_line: cursor_line.min(rows.saturating_sub(1)),
        cursor_column: cursor_column.min(cols.saturating_sub(1)),
        cursor_width,
        cursor_shape: renderable.cursor.shape,
        show_cursor,
        cursor_blinking,
        background: theme.background,
        cursor_color: theme.cursor_color,
        cursor_text: theme.cursor_text,
        selection_background: theme.selection_background,
        selection_foreground: theme.selection_foreground,
    }
}

fn cell_colors(
    foreground: AnsiColor,
    background: AnsiColor,
    flags: TermCellFlags,
    colors: &Colors,
    theme: &TerminalTheme,
) -> (TerminalColor, TerminalColor) {
    let mut fg = resolve_color(foreground, colors, theme);
    let mut bg = resolve_color(background, colors, theme);

    if flags.contains(TermCellFlags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }

    if flags.contains(TermCellFlags::DIM) {
        fg = fg.scale_alpha(0.8);
    }

    (fg, bg)
}

fn resolve_color(color: AnsiColor, colors: &Colors, theme: &TerminalTheme) -> TerminalColor {
    match color {
        AnsiColor::Named(named) => TerminalColor::from_rgb(
            colors[named].unwrap_or_else(|| fallback_named_color(named, theme)),
        ),
        AnsiColor::Spec(rgb) => TerminalColor::from_rgb(rgb),
        AnsiColor::Indexed(index) => TerminalColor::from_rgb(
            colors[index as usize].unwrap_or_else(|| fallback_indexed_color(index, theme)),
        ),
    }
}

fn fallback_named_color(named: NamedColor, theme: &TerminalTheme) -> Rgb {
    match named {
        NamedColor::Background => to_rgb(theme.background),
        NamedColor::Foreground | NamedColor::BrightForeground => to_rgb(theme.foreground),
        NamedColor::Cursor => to_rgb(theme.cursor_color),
        NamedColor::Black => to_rgb(theme.ansi[0]),
        NamedColor::Red => to_rgb(theme.ansi[1]),
        NamedColor::Green => to_rgb(theme.ansi[2]),
        NamedColor::Yellow => to_rgb(theme.ansi[3]),
        NamedColor::Blue => to_rgb(theme.ansi[4]),
        NamedColor::Magenta => to_rgb(theme.ansi[5]),
        NamedColor::Cyan => to_rgb(theme.ansi[6]),
        NamedColor::White => to_rgb(theme.ansi[7]),
        NamedColor::BrightBlack => to_rgb(theme.ansi[8]),
        NamedColor::BrightRed => to_rgb(theme.ansi[9]),
        NamedColor::BrightGreen => to_rgb(theme.ansi[10]),
        NamedColor::BrightYellow => to_rgb(theme.ansi[11]),
        NamedColor::BrightBlue => to_rgb(theme.ansi[12]),
        NamedColor::BrightMagenta => to_rgb(theme.ansi[13]),
        NamedColor::BrightCyan => to_rgb(theme.ansi[14]),
        NamedColor::BrightWhite => to_rgb(theme.ansi[15]),
        NamedColor::DimForeground => to_rgb(theme.foreground),
        NamedColor::DimBlack => to_rgb(theme.ansi[0]),
        NamedColor::DimRed => to_rgb(theme.ansi[1]),
        NamedColor::DimGreen => to_rgb(theme.ansi[2]),
        NamedColor::DimYellow => to_rgb(theme.ansi[3]),
        NamedColor::DimBlue => to_rgb(theme.ansi[4]),
        NamedColor::DimMagenta => to_rgb(theme.ansi[5]),
        NamedColor::DimCyan => to_rgb(theme.ansi[6]),
        NamedColor::DimWhite => to_rgb(theme.ansi[7]),
    }
}

fn fallback_indexed_color(index: u8, theme: &TerminalTheme) -> Rgb {
    if index < 16 {
        return to_rgb(theme.ansi[index as usize]);
    }

    if (16..=231).contains(&index) {
        let index = index - 16;
        let r = index / 36;
        let g = (index % 36) / 6;
        let b = index % 6;
        let component = |value: u8| if value == 0 { 0 } else { value * 40 + 55 };
        return Rgb {
            r: component(r),
            g: component(g),
            b: component(b),
        };
    }

    let gray = 8 + (index.saturating_sub(232) * 10);
    Rgb {
        r: gray,
        g: gray,
        b: gray,
    }
}

fn parse_hex_color(value: &str) -> Option<TerminalColor> {
    let value = value.trim().trim_start_matches('#');
    let (red, green, blue) = match value.len() {
        6 => (
            u8::from_str_radix(&value[0..2], 16).ok()?,
            u8::from_str_radix(&value[2..4], 16).ok()?,
            u8::from_str_radix(&value[4..6], 16).ok()?,
        ),
        _ => return None,
    };
    Some(TerminalColor {
        red,
        green,
        blue,
        alpha: 255,
    })
}

fn to_rgb(color: TerminalColor) -> Rgb {
    Rgb {
        r: color.red,
        g: color.green,
        b: color.blue,
    }
}

fn config_from_terminal(settings: &TerminalSettings) -> Config {
    let mut config = Config::default();
    config.default_cursor_style = alacritty_terminal::vte::ansi::CursorStyle {
        shape: match settings.cursor.shape.as_str() {
            "beam" => CursorShape::Beam,
            "underline" => CursorShape::Underline,
            _ => CursorShape::Block,
        },
        blinking: settings.cursor.blinking,
    };
    config.scrolling_history = settings.scrollback_lines.max(1);
    config
}

fn special_key_sequence(
    text: &str,
    modifiers: TerminalKeyModifiers,
    app_cursor: bool,
) -> Option<Vec<u8>> {
    let mut chars = text.chars();
    let key = chars.next()?;
    if chars.next().is_some() {
        return None;
    }

    match key {
        KEY_RETURN => Some(b"\r".to_vec()),
        KEY_BACKSPACE => Some(vec![0x7f]),
        KEY_TAB => Some(if modifiers.shift {
            b"\x1b[Z".to_vec()
        } else {
            b"\t".to_vec()
        }),
        KEY_BACKTAB => Some(b"\x1b[Z".to_vec()),
        KEY_ESCAPE => Some(vec![0x1b]),
        KEY_UP => Some(cursor_key(app_cursor, b'A', modifiers)),
        KEY_DOWN => Some(cursor_key(app_cursor, b'B', modifiers)),
        KEY_RIGHT => Some(cursor_key(app_cursor, b'C', modifiers)),
        KEY_LEFT => Some(cursor_key(app_cursor, b'D', modifiers)),
        KEY_HOME => Some(home_end_key(b'H', modifiers)),
        KEY_END => Some(home_end_key(b'F', modifiers)),
        KEY_INSERT => Some(csi_tilde_key(2, modifiers)),
        KEY_DELETE => Some(csi_tilde_key(3, modifiers)),
        KEY_PAGE_UP => Some(csi_tilde_key(5, modifiers)),
        KEY_PAGE_DOWN => Some(csi_tilde_key(6, modifiers)),
        KEY_F1 => Some(function_key(b'P', modifiers)),
        KEY_F2 => Some(function_key(b'Q', modifiers)),
        KEY_F3 => Some(function_key(b'R', modifiers)),
        KEY_F4 => Some(function_key(b'S', modifiers)),
        KEY_F5 => Some(csi_tilde_key(15, modifiers)),
        KEY_F6 => Some(csi_tilde_key(17, modifiers)),
        KEY_F7 => Some(csi_tilde_key(18, modifiers)),
        KEY_F8 => Some(csi_tilde_key(19, modifiers)),
        KEY_F9 => Some(csi_tilde_key(20, modifiers)),
        KEY_F10 => Some(csi_tilde_key(21, modifiers)),
        KEY_F11 => Some(csi_tilde_key(23, modifiers)),
        KEY_F12 => Some(csi_tilde_key(24, modifiers)),
        _ => None,
    }
}

fn cursor_key(app_cursor: bool, suffix: u8, modifiers: TerminalKeyModifiers) -> Vec<u8> {
    if let Some(modifier) = xterm_modifier(modifiers) {
        format!("\x1b[1;{modifier}{}", suffix as char).into_bytes()
    } else if app_cursor {
        vec![0x1b, b'O', suffix]
    } else {
        vec![0x1b, b'[', suffix]
    }
}

fn home_end_key(suffix: u8, modifiers: TerminalKeyModifiers) -> Vec<u8> {
    if let Some(modifier) = xterm_modifier(modifiers) {
        format!("\x1b[1;{modifier}{}", suffix as char).into_bytes()
    } else {
        vec![0x1b, b'[', suffix]
    }
}

fn csi_tilde_key(number: u8, modifiers: TerminalKeyModifiers) -> Vec<u8> {
    if let Some(modifier) = xterm_modifier(modifiers) {
        format!("\x1b[{number};{modifier}~").into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

fn function_key(ss3_suffix: u8, modifiers: TerminalKeyModifiers) -> Vec<u8> {
    if let Some(modifier) = xterm_modifier(modifiers) {
        format!("\x1b[1;{modifier}{}", ss3_suffix as char).into_bytes()
    } else {
        vec![0x1b, b'O', ss3_suffix]
    }
}

fn xterm_modifier(modifiers: TerminalKeyModifiers) -> Option<u8> {
    let mut modifier = 1;
    if modifiers.shift {
        modifier += 1;
    }
    if modifiers.alt {
        modifier += 2;
    }
    if modifiers.terminal_control() {
        modifier += 4;
    }

    (modifier > 1).then_some(modifier)
}

fn ctrl_sequence(text: &str) -> Option<Vec<u8>> {
    let mut chars = text.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }

    let byte = match ch {
        'a'..='z' => ch as u8 - b'a' + 1,
        'A'..='Z' => ch as u8 - b'A' + 1,
        ' ' | '@' | '`' => 0x00,
        '[' | '{' | KEY_ESCAPE => 0x1b,
        '\\' | '|' => 0x1c,
        ']' | '}' => 0x1d,
        '^' | '~' => 0x1e,
        '_' => 0x1f,
        '?' | KEY_BACKSPACE | KEY_DELETE => 0x7f,
        _ => return None,
    };

    Some(vec![byte])
}

fn selection_contains(selection: &TerminalSelection, cell: &TerminalCell) -> bool {
    let cell_start = (cell.line, cell.column);
    let cell_end = (cell.line, cell.column + cell.width.saturating_sub(1));
    let selection_start = (selection.start.line, selection.start.column);
    let selection_end = (selection.end.line, selection.end.column);

    cell_end >= selection_start && cell_start <= selection_end
}

fn underline_style(flags: TermCellFlags) -> Option<TerminalUnderlineStyle> {
    if flags.contains(TermCellFlags::DOUBLE_UNDERLINE) {
        Some(TerminalUnderlineStyle::Double)
    } else if flags.contains(TermCellFlags::UNDERCURL) {
        Some(TerminalUnderlineStyle::Curly)
    } else if flags.contains(TermCellFlags::DOTTED_UNDERLINE) {
        Some(TerminalUnderlineStyle::Dotted)
    } else if flags.contains(TermCellFlags::DASHED_UNDERLINE) {
        Some(TerminalUnderlineStyle::Dashed)
    } else if flags.contains(TermCellFlags::UNDERLINE) {
        Some(TerminalUnderlineStyle::Single)
    } else {
        None
    }
}

fn selection_at(
    snapshot: &TerminalSnapshot,
    point: TerminalPoint,
    classify: fn(&TerminalCell) -> WordCellClass,
) -> TerminalSelection {
    let Some(cell_index) = snapshot.cells.iter().position(|cell| {
        cell.line == point.line
            && cell.column <= point.column
            && point.column < cell.column + cell.width.max(1)
    }) else {
        return TerminalSelection {
            start: point,
            end: point,
        };
    };

    let cell = &snapshot.cells[cell_index];
    let class = classify(cell);
    let mut start = TerminalPoint {
        line: cell.line,
        column: cell.column,
    };
    let mut end = TerminalPoint {
        line: cell.line,
        column: cell.column + cell.width.max(1) - 1,
    };

    for candidate in snapshot.cells[..cell_index].iter().rev() {
        if candidate.line != cell.line
            || candidate.column + candidate.width.max(1) != start.column
            || classify(candidate) != class
        {
            break;
        }

        start.column = candidate.column;
    }

    for candidate in snapshot.cells[cell_index + 1..].iter() {
        if candidate.line != cell.line
            || candidate.column != end.column + 1
            || classify(candidate) != class
        {
            break;
        }

        end.column = candidate.column + candidate.width.max(1) - 1;
    }

    TerminalSelection { start, end }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordCellClass {
    Word,
    Whitespace,
    Symbol,
}

fn word_cell_class(cell: &TerminalCell) -> WordCellClass {
    if cell.text.chars().all(char::is_whitespace) {
        WordCellClass::Whitespace
    } else if cell.text.chars().all(is_plain_word_char) {
        WordCellClass::Word
    } else {
        WordCellClass::Symbol
    }
}

fn token_cell_class(cell: &TerminalCell) -> WordCellClass {
    if cell.text.chars().all(char::is_whitespace) {
        WordCellClass::Whitespace
    } else if cell.text.chars().all(is_terminal_word_char) {
        WordCellClass::Word
    } else {
        WordCellClass::Symbol
    }
}

fn is_plain_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '-'
}

fn is_terminal_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '\\' | '@' | '~' | ':')
}

const KEY_BACKSPACE: char = '\u{0008}';
const KEY_TAB: char = '\u{0009}';
const KEY_RETURN: char = '\u{000a}';
const KEY_ESCAPE: char = '\u{001b}';
const KEY_BACKTAB: char = '\u{0019}';
const KEY_DELETE: char = '\u{007f}';
const KEY_UP: char = '\u{F700}';
const KEY_DOWN: char = '\u{F701}';
const KEY_LEFT: char = '\u{F702}';
const KEY_RIGHT: char = '\u{F703}';
const KEY_F1: char = '\u{F704}';
const KEY_F2: char = '\u{F705}';
const KEY_F3: char = '\u{F706}';
const KEY_F4: char = '\u{F707}';
const KEY_F5: char = '\u{F708}';
const KEY_F6: char = '\u{F709}';
const KEY_F7: char = '\u{F70A}';
const KEY_F8: char = '\u{F70B}';
const KEY_F9: char = '\u{F70C}';
const KEY_F10: char = '\u{F70D}';
const KEY_F11: char = '\u{F70E}';
const KEY_F12: char = '\u{F70F}';
const KEY_INSERT: char = '\u{F727}';
const KEY_HOME: char = '\u{F729}';
const KEY_END: char = '\u{F72B}';
const KEY_PAGE_UP: char = '\u{F72C}';
const KEY_PAGE_DOWN: char = '\u{F72D}';

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(alt: bool, control: bool, shift: bool, meta: bool) -> TerminalKeyModifiers {
        TerminalKeyModifiers {
            alt,
            control,
            shift,
            meta,
        }
    }

    #[test]
    fn terminal_font_applies_native_metrics_with_line_height() {
        let settings = FontSettings {
            size: 13.0,
            line_height: 1.2,
            ..FontSettings::default()
        };
        let mut font = TerminalFont::from_settings(&settings);

        assert!(font.apply_native_metrics(7.5, 15.0, settings.line_height));
        assert_eq!(font.metrics.cell_width, 8.0);
        assert_eq!(font.metrics.cell_height, 18.0);
    }

    #[test]
    fn terminal_font_ignores_invalid_native_metrics() {
        let settings = FontSettings::default();
        let mut font = TerminalFont::from_settings(&settings);
        let original = font.metrics;

        assert!(!font.apply_native_metrics(0.0, 15.0, settings.line_height));
        assert_eq!(font.metrics.cell_width, original.cell_width);
        assert_eq!(font.metrics.cell_height, original.cell_height);
        assert!(!font.apply_native_metrics(7.5, 0.0, settings.line_height));
        assert_eq!(font.metrics.cell_width, original.cell_width);
        assert_eq!(font.metrics.cell_height, original.cell_height);
    }

    #[test]
    fn ctrl_sequence_encodes_letters() {
        assert_eq!(ctrl_sequence("c"), Some(vec![0x03]));
        assert_eq!(ctrl_sequence("C"), Some(vec![0x03]));
    }

    #[test]
    fn special_key_sequence_encodes_shift_alt_arrow() {
        assert_eq!(
            special_key_sequence("\u{F700}", modifiers(true, false, true, false), false),
            Some(b"\x1b[1;4A".to_vec())
        );
    }

    #[test]
    fn special_key_sequence_encodes_basic_named_keys() {
        let none = TerminalKeyModifiers::default();

        assert_eq!(
            special_key_sequence("\u{000a}", none, false),
            Some(b"\r".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{0008}", none, false),
            Some(vec![0x7f])
        );
        assert_eq!(
            special_key_sequence("\u{001b}", none, false),
            Some(vec![0x1b])
        );
        assert_eq!(
            special_key_sequence("\u{F729}", none, false),
            Some(b"\x1b[H".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{F72B}", none, false),
            Some(b"\x1b[F".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{F727}", none, false),
            Some(b"\x1b[2~".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{007f}", none, false),
            Some(b"\x1b[3~".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{F72C}", none, false),
            Some(b"\x1b[5~".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{F72D}", none, false),
            Some(b"\x1b[6~".to_vec())
        );
    }

    #[test]
    fn special_key_sequence_encodes_tab_variants() {
        let none = TerminalKeyModifiers::default();

        assert_eq!(
            special_key_sequence("\u{0009}", none, false),
            Some(b"\t".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{0009}", modifiers(false, false, true, false), false),
            Some(b"\x1b[Z".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{0019}", none, false),
            Some(b"\x1b[Z".to_vec())
        );
    }

    #[test]
    fn special_key_sequence_respects_application_cursor_mode() {
        let none = TerminalKeyModifiers::default();

        assert_eq!(
            special_key_sequence("\u{F700}", none, false),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{F700}", none, true),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{F700}", modifiers(false, false, true, false), true),
            Some(b"\x1b[1;2A".to_vec())
        );
    }

    #[test]
    fn special_key_sequence_encodes_control_arrow() {
        assert_eq!(
            special_key_sequence("\u{F702}", platform_terminal_control_modifiers(), false),
            Some(b"\x1b[1;5D".to_vec())
        );
    }

    #[test]
    fn special_key_sequence_encodes_control_delete() {
        assert_eq!(
            special_key_sequence("\u{007f}", platform_terminal_control_modifiers(), false),
            Some(b"\x1b[3;5~".to_vec())
        );
    }

    #[test]
    fn special_key_sequence_encodes_function_keys() {
        assert_eq!(
            special_key_sequence("\u{F708}", TerminalKeyModifiers::default(), false),
            Some(b"\x1b[15~".to_vec())
        );
    }

    #[test]
    fn special_key_sequence_encodes_modified_function_keys() {
        assert_eq!(
            special_key_sequence("\u{F704}", modifiers(true, false, true, false), false),
            Some(b"\x1b[1;4P".to_vec())
        );
        assert_eq!(
            special_key_sequence("\u{F708}", platform_terminal_control_modifiers(), false),
            Some(b"\x1b[15;5~".to_vec())
        );
    }

    #[test]
    fn encode_key_text_falls_through_from_control_to_special_keys() {
        let settings = TerminalSettings::default();
        let terminal = TerminalView::new(80, 24, &settings);

        assert_eq!(
            terminal.encode_key_text("\u{F702}", platform_terminal_control_modifiers()),
            Some(b"\x1b[1;5D".to_vec())
        );
    }

    #[test]
    fn encode_key_text_keeps_control_characters_for_text() {
        let settings = TerminalSettings::default();
        let terminal = TerminalView::new(80, 24, &settings);

        assert_eq!(
            terminal.encode_key_text("c", platform_terminal_control_modifiers()),
            Some(vec![0x03])
        );
    }

    #[test]
    fn command_modifier_is_not_terminal_control() {
        #[cfg(target_os = "macos")]
        let command = modifiers(false, true, false, false);
        #[cfg(not(target_os = "macos"))]
        let command = modifiers(false, false, false, true);

        assert!(command.command_modifier());
        assert!(!command.terminal_control());
    }

    #[test]
    fn platform_terminal_control_is_distinct_from_command() {
        let control = platform_terminal_control_modifiers();

        assert!(control.terminal_control());
        assert!(!control.command_modifier());
    }

    #[test]
    fn selection_contents_preserves_rows_and_gaps() {
        let snapshot = test_snapshot(vec![
            test_cell("a", 0, 0),
            test_cell("b", 0, 2),
            test_cell("c", 1, 1),
        ]);
        let selection = TerminalSelection {
            start: TerminalPoint { line: 0, column: 0 },
            end: TerminalPoint { line: 1, column: 1 },
        };

        assert_eq!(
            selection_contents(&snapshot, Some(&selection)),
            Some("a b\n c".into())
        );
    }

    #[test]
    fn selection_contents_preserves_real_cjk_wide_cells() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(80, 24, &settings);
        terminal.feed("界A".as_bytes());
        let snapshot = terminal.snapshot(&theme);

        assert_eq!(
            selection_contents(
                &snapshot,
                Some(&TerminalSelection {
                    start: TerminalPoint { line: 0, column: 1 },
                    end: TerminalPoint { line: 0, column: 2 },
                })
            ),
            Some("界A".into())
        );
    }

    #[test]
    fn paste_text_normalizes_newlines_without_bracketed_mode() {
        let settings = TerminalSettings::default();
        let terminal = TerminalView::new(80, 24, &settings);

        assert_eq!(terminal.encode_text_input("a\r\nb\nc"), b"a\rb\rc".to_vec());
    }

    #[test]
    fn paste_text_uses_bracketed_paste_for_multiline_content() {
        let settings = TerminalSettings::default();
        let mut terminal = TerminalView::new(80, 24, &settings);
        terminal.feed(b"\x1b[?2004h");

        assert_eq!(
            terminal.encode_text_input("a\r\nb"),
            b"\x1b[200~a\nb\x1b[201~".to_vec()
        );
    }

    #[test]
    fn alternate_screen_scroll_writes_arrow_keys_to_outbound_session() {
        let settings = TerminalSettings::default();
        let mut terminal = TerminalView::new(80, 24, &settings);
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        terminal.set_outbound(command_tx);

        terminal.feed(b"\x1b[?1049h");
        terminal.handle_scroll(1, TerminalPoint { line: 0, column: 0 });

        match command_rx.try_recv() {
            Ok(SessionCommand::Input(bytes)) => assert_eq!(bytes, b"\x1b[A".to_vec()),
            other => panic!("expected alternate scroll input, got {other:?}"),
        }
    }

    #[test]
    fn scroll_to_bottom_resets_scrollback_display_offset() {
        let settings = TerminalSettings::default();
        let mut terminal = TerminalView::new(80, 5, &settings);
        for index in 0..20 {
            terminal.feed(format!("line {index}\r\n").as_bytes());
        }

        terminal.handle_scroll(3, TerminalPoint { line: 0, column: 0 });
        assert!(terminal.display_offset() > 0);

        terminal.scroll_to_bottom();
        assert_eq!(terminal.display_offset(), 0);
    }

    #[test]
    fn sgr_mouse_mode_reports_press_drag_and_release_to_outbound_session() {
        let settings = TerminalSettings::default();
        let mut terminal = TerminalView::new(80, 24, &settings);
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        terminal.set_outbound(command_tx);

        terminal.feed(b"\x1b[?1002;1006h");

        assert!(terminal.handle_mouse_press(TerminalPoint { line: 1, column: 2 }));
        assert!(terminal.handle_mouse_drag(TerminalPoint { line: 2, column: 4 }));
        assert!(terminal.handle_mouse_release(TerminalPoint { line: 2, column: 4 }));

        assert_eq!(next_input(&mut command_rx), b"\x1b[<0;3;2M".to_vec());
        assert_eq!(next_input(&mut command_rx), b"\x1b[<32;5;3M".to_vec());
        assert_eq!(next_input(&mut command_rx), b"\x1b[<0;5;3m".to_vec());
    }

    #[test]
    fn legacy_mouse_mode_reports_press_and_release_to_outbound_session() {
        let settings = TerminalSettings::default();
        let mut terminal = TerminalView::new(80, 24, &settings);
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        terminal.set_outbound(command_tx);

        terminal.feed(b"\x1b[?1000h");

        assert!(terminal.handle_mouse_press(TerminalPoint { line: 0, column: 0 }));
        assert!(terminal.handle_mouse_release(TerminalPoint { line: 0, column: 0 }));

        assert_eq!(
            next_input(&mut command_rx),
            vec![0x1b, b'[', b'M', 32, 33, 33]
        );
        assert_eq!(
            next_input(&mut command_rx),
            vec![0x1b, b'[', b'M', 35, 33, 33]
        );
    }

    #[test]
    fn focus_reporting_writes_focus_in_and_out_to_outbound_session() {
        let settings = TerminalSettings::default();
        let mut terminal = TerminalView::new(80, 24, &settings);
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        terminal.set_outbound(command_tx);

        terminal.feed(b"\x1b[?1004h");

        assert!(terminal.handle_focus_change(true));
        assert!(terminal.handle_focus_change(false));

        assert_eq!(next_input(&mut command_rx), b"\x1b[I".to_vec());
        assert_eq!(next_input(&mut command_rx), b"\x1b[O".to_vec());
    }

    #[test]
    fn focus_change_without_reporting_does_not_write_to_outbound_session() {
        let settings = TerminalSettings::default();
        let mut terminal = TerminalView::new(80, 24, &settings);
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        terminal.set_outbound(command_tx);

        assert!(!terminal.handle_focus_change(true));
        assert!(command_rx.try_recv().is_err());
    }

    #[test]
    fn snapshot_applies_inverse_video_to_cell_colors() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(80, 24, &settings);
        terminal.feed(b"\x1b[31;44;7mA");

        let snapshot = terminal.snapshot(&theme);
        let cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "A")
            .expect("rendered inverse cell");

        assert_eq!(cell.fg, theme.ansi[4]);
        assert_eq!(cell.bg, theme.ansi[1]);
    }

    #[test]
    fn snapshot_dims_final_foreground_after_inverse_video() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(80, 24, &settings);
        terminal.feed(b"\x1b[31;44;2;7mA");

        let snapshot = terminal.snapshot(&theme);
        let cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "A")
            .expect("rendered dim inverse cell");

        assert_eq!(cell.fg, theme.ansi[4].scale_alpha(0.8));
        assert_eq!(cell.bg, theme.ansi[1]);
    }

    #[test]
    fn snapshot_marks_cjk_cells_as_double_width() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(80, 24, &settings);
        terminal.feed("界".as_bytes());

        let snapshot = terminal.snapshot(&theme);
        let cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "界")
            .expect("rendered CJK wide cell");

        assert_eq!(cell.line, 0);
        assert_eq!(cell.column, 0);
        assert_eq!(cell.width, 2);
        assert!(
            !snapshot
                .cells
                .iter()
                .any(|cell| cell.line == 0 && cell.column == 1 && cell.text == "界")
        );
    }

    #[test]
    fn snapshot_uses_double_width_cursor_on_cjk_cell() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(80, 24, &settings);
        terminal.feed("界\x1b[D".as_bytes());

        let snapshot = terminal.snapshot(&theme);

        assert_eq!(snapshot.cursor_line, 0);
        assert_eq!(snapshot.cursor_column, 0);
        assert_eq!(snapshot.cursor_width, 2);
    }

    #[test]
    fn word_selection_stops_at_terminal_token_symbols() {
        let snapshot = test_snapshot(test_cells_from_line("admin@10.0.1.10:/tmp", 0));

        assert_eq!(
            selection_at(
                &snapshot,
                TerminalPoint { line: 0, column: 6 },
                word_cell_class
            ),
            TerminalSelection {
                start: TerminalPoint { line: 0, column: 6 },
                end: TerminalPoint { line: 0, column: 7 },
            }
        );
    }

    #[test]
    fn token_selection_keeps_terminal_paths_together() {
        let snapshot = test_snapshot(test_cells_from_line("admin@10.0.1.10:/tmp", 0));

        assert_eq!(
            selection_at(
                &snapshot,
                TerminalPoint { line: 0, column: 6 },
                token_cell_class
            ),
            TerminalSelection {
                start: TerminalPoint { line: 0, column: 0 },
                end: TerminalPoint {
                    line: 0,
                    column: "admin@10.0.1.10:/tmp".len() - 1,
                },
            }
        );
    }

    #[test]
    fn selection_at_missing_cell_returns_point_selection() {
        let snapshot = test_snapshot(vec![test_cell("a", 0, 0)]);

        assert_eq!(
            selection_at(
                &snapshot,
                TerminalPoint { line: 0, column: 5 },
                word_cell_class
            ),
            TerminalSelection {
                start: TerminalPoint { line: 0, column: 5 },
                end: TerminalPoint { line: 0, column: 5 },
            }
        );
    }

    fn test_snapshot(cells: Vec<TerminalCell>) -> TerminalSnapshot {
        TerminalSnapshot {
            cells,
            cursor_line: 0,
            cursor_column: 0,
            cursor_width: 1,
            cursor_shape: CursorShape::Block,
            show_cursor: false,
            cursor_blinking: false,
            background: test_color(),
            cursor_color: test_color(),
            cursor_text: test_color(),
            selection_background: test_color(),
            selection_foreground: test_color(),
        }
    }

    fn test_cells_from_line(text: &str, line: usize) -> Vec<TerminalCell> {
        text.chars()
            .enumerate()
            .map(|(column, ch)| test_cell(&ch.to_string(), line, column))
            .collect()
    }

    fn test_cell(text: &str, line: usize, column: usize) -> TerminalCell {
        TerminalCell {
            text: text.into(),
            fg: test_color(),
            bg: test_color(),
            underline: None,
            underline_color: test_color(),
            width: 1,
            bold: false,
            italic: false,
            strikeout: false,
            dim: false,
            hidden: false,
            line,
            column,
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

    fn next_input(command_rx: &mut mpsc::UnboundedReceiver<SessionCommand>) -> Vec<u8> {
        match command_rx.try_recv() {
            Ok(SessionCommand::Input(bytes)) => bytes,
            other => panic!("expected input command, got {other:?}"),
        }
    }

    fn platform_terminal_control_modifiers() -> TerminalKeyModifiers {
        #[cfg(target_os = "macos")]
        return modifiers(false, false, false, true);
        #[cfg(not(target_os = "macos"))]
        return modifiers(false, true, false, false);
    }
}
