use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::cell::Flags as TermCellFlags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, RenderableContent, Term, TermMode, point_to_viewport};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb};
use bytemuck::{Pod, Zeroable};
use iced::advanced::graphics::text as graphics_text;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer as advanced_renderer;
use iced::advanced::widget::tree;
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Shell, input_method};
use iced::font::{Style as FontStyle, Weight as FontWeight};
use iced::keyboard::{Key, Modifiers, key};
use iced::mouse;
use iced::time::{Duration, Instant};
use iced::wgpu;
use iced::widget::shader::{Pipeline as ShaderPipeline, Primitive as ShaderPrimitive, Viewport};
use iced::{Color, Element, Event as IcedEvent, Length, Point, Rectangle, Size};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;
use tokio::sync::mpsc;
use wgpu::util::DeviceExt;

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
    pub background: Color,
    pub foreground: Color,
    pub cursor_color: Color,
    pub cursor_text: Color,
    pub selection_background: Color,
    pub selection_foreground: Color,
    pub ansi: [Color; 16],
}

#[derive(Debug, Clone)]
pub struct TerminalFont {
    pub size: f32,
    pub line_height: f32,
    pub thicken: f32,
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
    pub fg: Color,
    pub bg: Color,
    pub underline: Option<UnderlineStyle>,
    pub underline_color: Color,
    pub width: usize,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
    pub hidden: bool,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnderlineStyle {
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
    pub background: Color,
    pub cursor_color: Color,
    pub cursor_text: Color,
    pub selection_background: Color,
    pub selection_foreground: Color,
}

#[derive(Debug, Clone)]
pub struct TerminalSelection {
    pub start: TerminalPoint,
    pub end: TerminalPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalPoint {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub enum TerminalCanvasEvent {
    SelectionStarted(TerminalPoint),
    SelectionUpdated(TerminalPoint),
    Scrolled { lines: i32, point: TerminalPoint },
    Resized { cols: usize, rows: usize },
}

pub struct TerminalAtlasState {
    dragging: bool,
    last_point: Option<TerminalPoint>,
    cursor_visible: bool,
    cursor_blink_started_at: Instant,
    last_cursor_key: Option<CursorBlinkKey>,
    last_terminal_size: Option<(usize, usize)>,
}

impl Default for TerminalAtlasState {
    fn default() -> Self {
        Self {
            dragging: false,
            last_point: None,
            cursor_visible: true,
            cursor_blink_started_at: Instant::now(),
            last_cursor_key: None,
            last_terminal_size: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CursorBlinkKey {
    line: usize,
    column: usize,
    width: usize,
    shape: CursorShape,
    show_cursor: bool,
    blinking: bool,
    focused: bool,
}

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(600);

pub struct TerminalAtlas<Message> {
    snapshot: TerminalSnapshot,
    selection: Option<TerminalSelection>,
    font: TerminalFont,
    atlas: Arc<Mutex<GlyphAtlas>>,
    scale_factor: f32,
    focused: bool,
    on_event: Arc<dyn Fn(TerminalCanvasEvent) -> Message + Send + Sync>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GlyphKey {
    pub text: String,
    pub family_name: String,
    pub cell_columns: u8,
    pub font_size_bits: u32,
    pub line_height_bits: u32,
    pub thicken_bits: u32,
    pub scale_factor_bits: u32,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    pub page_index: usize,
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
}

#[derive(Debug)]
struct MaskAtlasPage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    version: u64,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

#[derive(Debug)]
pub struct GlyphAtlas {
    glyphs: HashMap<GlyphKey, RasterizedGlyph>,
    pages: Vec<MaskAtlasPage>,
    swash_cache: graphics_text::cosmic_text::SwashCache,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ViewportUniform {
    size: [f32; 2],
    _padding: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct RectInstance {
    rect: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GlyphInstance {
    rect: [f32; 4],
    uv_rect: [f32; 4],
    color: [f32; 4],
    extras: [f32; 4],
}

#[derive(Debug, Clone)]
struct PreparedGlyph {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    page_index: usize,
    atlas_x: u32,
    atlas_y: u32,
    atlas_width: u32,
    atlas_height: u32,
    color: Color,
}

#[derive(Debug, Default)]
struct TerminalAtlasPipeline {
    rect_pipeline: Option<wgpu::RenderPipeline>,
    glyph_pipeline: Option<wgpu::RenderPipeline>,
    viewport_buffer: Option<wgpu::Buffer>,
    viewport_bind_group: Option<wgpu::BindGroup>,
    viewport_bind_group_layout: Option<wgpu::BindGroupLayout>,
    atlas_bind_group_layout: Option<wgpu::BindGroupLayout>,
    atlas_sampler: Option<wgpu::Sampler>,
    atlas_texture: Option<wgpu::Texture>,
    atlas_texture_view: Option<wgpu::TextureView>,
    atlas_bind_group: Option<wgpu::BindGroup>,
    atlas_layer_count: u32,
    atlas_versions: Vec<u64>,
    text_rect_buffer: Option<wgpu::Buffer>,
    text_rect_count: u32,
    glyph_buffer: Option<wgpu::Buffer>,
    glyph_count: u32,
    overlay_rect_buffer: Option<wgpu::Buffer>,
    overlay_rect_count: u32,
    text_cache_key: Option<u64>,
    overlay_cache_key: Option<u64>,
    viewport_size: [f32; 2],
}

#[derive(Debug, Clone)]
struct TerminalAtlasPrimitive {
    snapshot: TerminalSnapshot,
    selection: Option<TerminalSelection>,
    font: TerminalFont,
    atlas: Arc<Mutex<GlyphAtlas>>,
    scale_factor: f32,
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

    pub fn reset(&mut self, settings: &TerminalSettings) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        self.event_proxy = TerminalEventProxy {
            outbound: self.event_proxy.outbound.clone(),
            events: event_tx,
        };
        self.term = Term::new(
            config_from_terminal(settings),
            &TermSize::new(self.cols, self.rows),
            self.event_proxy.clone(),
        );
        self.parser = Processor::new();
        self.event_rx = event_rx;
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

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols.max(2);
        self.rows = rows.max(2);
        self.term.resize(TermSize::new(self.cols, self.rows));
    }

    pub fn encode_key(
        &self,
        key: Key,
        modifiers: Modifiers,
        text: Option<String>,
    ) -> Option<Vec<u8>> {
        if modifiers.command() {
            return None;
        }

        if modifiers.control() {
            return ctrl_sequence(&key);
        }

        if let Some(text) = text {
            let mut bytes = Vec::new();
            if modifiers.alt() {
                bytes.push(0x1b);
            }
            bytes.extend_from_slice(text.as_bytes());
            return Some(bytes);
        }

        let app_cursor = self.term.mode().contains(TermMode::APP_CURSOR);

        match key.as_ref() {
            Key::Named(key::Named::Enter) => Some(b"\r".to_vec()),
            Key::Named(key::Named::Tab) => Some(if modifiers.shift() {
                b"\x1b[Z".to_vec()
            } else {
                b"\t".to_vec()
            }),
            Key::Named(key::Named::Backspace) => Some(vec![0x7f]),
            Key::Named(key::Named::Escape) => Some(vec![0x1b]),
            Key::Named(key::Named::ArrowUp) => Some(cursor_key(
                app_cursor,
                b'A',
                modifiers.shift(),
                modifiers.alt(),
            )),
            Key::Named(key::Named::ArrowDown) => Some(cursor_key(
                app_cursor,
                b'B',
                modifiers.shift(),
                modifiers.alt(),
            )),
            Key::Named(key::Named::ArrowRight) => Some(cursor_key(
                app_cursor,
                b'C',
                modifiers.shift(),
                modifiers.alt(),
            )),
            Key::Named(key::Named::ArrowLeft) => Some(cursor_key(
                app_cursor,
                b'D',
                modifiers.shift(),
                modifiers.alt(),
            )),
            Key::Named(key::Named::Home) => Some(b"\x1b[H".to_vec()),
            Key::Named(key::Named::End) => Some(b"\x1b[F".to_vec()),
            Key::Named(key::Named::Insert) => Some(b"\x1b[2~".to_vec()),
            Key::Named(key::Named::Delete) => Some(b"\x1b[3~".to_vec()),
            Key::Named(key::Named::PageUp) => Some(b"\x1b[5~".to_vec()),
            Key::Named(key::Named::PageDown) => Some(b"\x1b[6~".to_vec()),
            Key::Named(key::Named::F1) => Some(b"\x1bOP".to_vec()),
            Key::Named(key::Named::F2) => Some(b"\x1bOQ".to_vec()),
            Key::Named(key::Named::F3) => Some(b"\x1bOR".to_vec()),
            Key::Named(key::Named::F4) => Some(b"\x1bOS".to_vec()),
            Key::Named(key::Named::F5) => Some(b"\x1b[15~".to_vec()),
            Key::Named(key::Named::F6) => Some(b"\x1b[17~".to_vec()),
            Key::Named(key::Named::F7) => Some(b"\x1b[18~".to_vec()),
            Key::Named(key::Named::F8) => Some(b"\x1b[19~".to_vec()),
            Key::Named(key::Named::F9) => Some(b"\x1b[20~".to_vec()),
            Key::Named(key::Named::F10) => Some(b"\x1b[21~".to_vec()),
            Key::Named(key::Named::F11) => Some(b"\x1b[23~".to_vec()),
            Key::Named(key::Named::F12) => Some(b"\x1b[24~".to_vec()),
            _ => None,
        }
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

    fn send_alternate_scroll(&self, delta: i32, app_cursor: bool) {
        let sequence = if delta > 0 {
            cursor_key(app_cursor, b'A', false, false)
        } else {
            cursor_key(app_cursor, b'B', false, false)
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
                    (32 + column.min(223)) as u8,
                    (32 + line.min(223)) as u8,
                ]
            };

            self.event_proxy.send_input(SessionCommand::Input(payload));
        }
    }
}

impl<Message> Clone for TerminalAtlas<Message> {
    fn clone(&self) -> Self {
        Self {
            snapshot: self.snapshot.clone(),
            selection: self.selection.clone(),
            font: self.font.clone(),
            atlas: self.atlas.clone(),
            scale_factor: self.scale_factor,
            focused: self.focused,
            on_event: self.on_event.clone(),
        }
    }
}

impl<Message: 'static> TerminalAtlas<Message> {
    pub fn new(
        snapshot: TerminalSnapshot,
        selection: Option<TerminalSelection>,
        font: TerminalFont,
        atlas: Arc<Mutex<GlyphAtlas>>,
        scale_factor: f32,
        focused: bool,
        on_event: Arc<dyn Fn(TerminalCanvasEvent) -> Message + Send + Sync>,
    ) -> Self {
        Self {
            snapshot,
            selection,
            font,
            atlas,
            scale_factor: scale_factor.max(1.0),
            focused,
            on_event,
        }
    }

    pub fn element(self) -> Element<'static, Message> {
        Element::new(self)
    }

    fn point_at(&self, bounds: Rectangle, point: Point) -> Option<TerminalPoint> {
        if point.x < 0.0 || point.y < 0.0 || point.x > bounds.width || point.y > bounds.height {
            return None;
        }

        let column = (point.x / self.font.metrics.cell_width).floor().max(0.0) as usize;
        let line = (point.y / self.font.metrics.cell_height).floor().max(0.0) as usize;

        Some(TerminalPoint { line, column })
    }

    fn point_at_clamped(&self, bounds: Rectangle, point: Point) -> TerminalPoint {
        let max_x = (bounds.width - 0.01).max(0.0);
        let max_y = (bounds.height - 0.01).max(0.0);
        let clamped = Point::new(point.x.clamp(0.0, max_x), point.y.clamp(0.0, max_y));

        self.point_at(bounds, clamped)
            .unwrap_or(TerminalPoint { line: 0, column: 0 })
    }

    fn size_for_bounds(&self, bounds: Rectangle) -> (usize, usize) {
        let cols = (bounds.width / self.font.metrics.cell_width.max(1.0)).floor() as usize;
        let rows = (bounds.height / self.font.metrics.cell_height.max(1.0)).floor() as usize;

        (cols.max(2), rows.max(2))
    }
}

impl<Message, Theme, RendererType> Widget<Message, Theme, RendererType> for TerminalAtlas<Message>
where
    Message: 'static,
    RendererType: PrimitiveRenderer,
{
    fn tag(&self) -> tree::Tag {
        struct Tag<T>(T);
        tree::Tag::of::<Tag<TerminalAtlasState>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(TerminalAtlasState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &RendererType,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &IcedEvent,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &RendererType,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<TerminalAtlasState>();

        match event {
            IcedEvent::Window(iced::window::Event::RedrawRequested(now)) => {
                let terminal_size = self.size_for_bounds(bounds);
                if state.last_terminal_size != Some(terminal_size) {
                    state.last_terminal_size = Some(terminal_size);
                    shell.publish((self.on_event)(TerminalCanvasEvent::Resized {
                        cols: terminal_size.0,
                        rows: terminal_size.1,
                    }));
                }

                if !self.focused {
                    return;
                }

                let cursor_key = CursorBlinkKey {
                    line: self.snapshot.cursor_line,
                    column: self.snapshot.cursor_column,
                    width: self.snapshot.cursor_width,
                    shape: self.snapshot.cursor_shape,
                    show_cursor: self.snapshot.show_cursor,
                    blinking: self.snapshot.cursor_blinking,
                    focused: self.focused,
                };
                if state.last_cursor_key != Some(cursor_key) {
                    state.last_cursor_key = Some(cursor_key);
                    state.cursor_visible = true;
                    state.cursor_blink_started_at = *now;
                }
                if self.snapshot.show_cursor && self.snapshot.cursor_blinking {
                    let elapsed = now.saturating_duration_since(state.cursor_blink_started_at);
                    let phase = (elapsed.as_millis() / CURSOR_BLINK_INTERVAL.as_millis()) % 2;
                    state.cursor_visible = phase == 0;
                    let millis_until_redraw = CURSOR_BLINK_INTERVAL.as_millis()
                        - elapsed.as_millis() % CURSOR_BLINK_INTERVAL.as_millis();
                    shell.request_redraw_at(
                        *now + Duration::from_millis(millis_until_redraw as u64),
                    );
                } else {
                    state.cursor_visible = true;
                }

                let cursor = Rectangle {
                    x: bounds.x + self.snapshot.cursor_column as f32 * self.font.metrics.cell_width,
                    y: bounds.y + self.snapshot.cursor_line as f32 * self.font.metrics.cell_height,
                    width: (self.snapshot.cursor_width.max(1) as f32
                        * self.font.metrics.cell_width)
                        .max(1.0),
                    height: self.font.metrics.cell_height.max(1.0),
                };

                shell.request_input_method(&input_method::InputMethod::Enabled {
                    cursor,
                    purpose: input_method::Purpose::Terminal,
                    preedit: None::<input_method::Preedit<String>>,
                });
            }
            IcedEvent::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(point) = cursor.position_in(bounds) else {
                    return;
                };
                let Some(terminal_point) = self.point_at(bounds, point) else {
                    return;
                };

                state.dragging = true;
                state.last_point = Some(terminal_point);
                state.cursor_visible = true;
                state.cursor_blink_started_at = Instant::now();
                shell.publish((self.on_event)(TerminalCanvasEvent::SelectionStarted(
                    terminal_point,
                )));
                shell.capture_event();
            }
            IcedEvent::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let Some(point) = cursor.position_from(Point::new(bounds.x, bounds.y)) else {
                    return;
                };
                let terminal_point = self.point_at_clamped(bounds, point);

                if state.last_point == Some(terminal_point) {
                    shell.capture_event();
                    return;
                }

                state.last_point = Some(terminal_point);
                shell.publish((self.on_event)(TerminalCanvasEvent::SelectionUpdated(
                    terminal_point,
                )));
                shell.capture_event();
            }
            IcedEvent::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.dragging =>
            {
                state.dragging = false;
                state.last_point = None;
                shell.capture_event();
            }
            IcedEvent::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(_point) = cursor.position_in(bounds) else {
                    return;
                };

                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => (*y).round() as i32,
                    mouse::ScrollDelta::Pixels { y, .. } => {
                        let lines = *y / self.font.metrics.cell_height.max(1.0);
                        if lines.abs() < 1.0 {
                            lines.signum() as i32
                        } else {
                            lines.round() as i32
                        }
                    }
                };

                if lines != 0 {
                    let Some(point) = self.point_at(bounds, _point) else {
                        return;
                    };
                    shell.publish((self.on_event)(TerminalCanvasEvent::Scrolled {
                        lines,
                        point,
                    }));
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut RendererType,
        _theme: &Theme,
        _style: &advanced_renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<TerminalAtlasState>();
        let mut snapshot = self.snapshot.clone();
        if !self.focused {
            snapshot.show_cursor = false;
        } else if snapshot.cursor_blinking && !state.cursor_visible {
            snapshot.show_cursor = false;
        }
        renderer.draw_primitive(
            layout.bounds(),
            TerminalAtlasPrimitive {
                snapshot,
                selection: self.selection.clone(),
                font: self.font.clone(),
                atlas: self.atlas.clone(),
                scale_factor: self.scale_factor,
            },
        );
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &RendererType,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

impl TerminalAtlasPipeline {
    fn ensure_resources(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.rect_pipeline.is_some() {
            return;
        }

        let viewport_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("timon.terminal.viewport-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ViewportUniform>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timon.terminal.viewport-buffer"),
            size: std::mem::size_of::<ViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("timon.terminal.viewport-bind-group"),
            layout: &viewport_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("timon.terminal.atlas-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("timon.terminal.atlas-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("timon.terminal.atlas-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("terminal_atlas.wgsl").into()),
        });

        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("timon.terminal.rect-pipeline-layout"),
            bind_group_layouts: &[&viewport_bind_group_layout],
            push_constant_ranges: &[],
        });

        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("timon.terminal.rect-pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("rect_vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("rect_fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("timon.terminal.glyph-pipeline-layout"),
                bind_group_layouts: &[&viewport_bind_group_layout, &atlas_bind_group_layout],
                push_constant_ranges: &[],
            });

        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("timon.terminal.glyph-pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("glyph_vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x4,
                        3 => Float32x4
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("glyph_fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.rect_pipeline = Some(rect_pipeline);
        self.glyph_pipeline = Some(glyph_pipeline);
        self.viewport_buffer = Some(viewport_buffer);
        self.viewport_bind_group = Some(viewport_bind_group);
        self.viewport_bind_group_layout = Some(viewport_bind_group_layout);
        self.atlas_bind_group_layout = Some(atlas_bind_group_layout);
        self.atlas_sampler = Some(atlas_sampler);
    }

    fn sync_viewport(&mut self, queue: &wgpu::Queue, size: [f32; 2]) {
        if self.viewport_size == size {
            return;
        }

        self.viewport_size = size;
        if let Some(buffer) = &self.viewport_buffer {
            let uniform = ViewportUniform {
                size,
                _padding: [0.0; 2],
            };
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&uniform));
        }
    }

    fn sync_atlas_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &GlyphAtlas,
    ) {
        if atlas.pages.is_empty() {
            self.atlas_texture = None;
            self.atlas_texture_view = None;
            self.atlas_bind_group = None;
            self.atlas_layer_count = 0;
            self.atlas_versions.clear();
            return;
        }

        let layer_count = atlas.pages.len() as u32;
        let page_width = atlas.pages[0].width;
        let page_height = atlas.pages[0].height;
        let needs_recreate = self.atlas_layer_count != layer_count || self.atlas_texture.is_none();

        if needs_recreate {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("timon.terminal.atlas-texture"),
                size: wgpu::Extent3d {
                    width: page_width,
                    height: page_height,
                    depth_or_array_layers: layer_count,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("timon.terminal.atlas-texture-view"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("timon.terminal.atlas-bind-group"),
                layout: self
                    .atlas_bind_group_layout
                    .as_ref()
                    .expect("atlas bind group layout should exist"),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(
                            self.atlas_sampler
                                .as_ref()
                                .expect("atlas sampler should exist"),
                        ),
                    },
                ],
            });

            self.atlas_texture = Some(texture);
            self.atlas_texture_view = Some(texture_view);
            self.atlas_bind_group = Some(bind_group);
            self.atlas_layer_count = layer_count;
            self.atlas_versions = vec![u64::MAX; layer_count as usize];
        }

        let Some(texture) = &self.atlas_texture else {
            return;
        };

        for (index, page) in atlas.pages.iter().enumerate() {
            if self.atlas_versions.get(index) == Some(&page.version) {
                continue;
            }

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &page.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(page.width),
                    rows_per_image: Some(page.height),
                },
                wgpu::Extent3d {
                    width: page.width,
                    height: page.height,
                    depth_or_array_layers: 1,
                },
            );

            if let Some(version) = self.atlas_versions.get_mut(index) {
                *version = page.version;
            }
        }
    }

    fn sync_rect_buffer(
        buffer: &mut Option<wgpu::Buffer>,
        count: &mut u32,
        device: &wgpu::Device,
        instances: &[RectInstance],
        label: &str,
    ) {
        if instances.is_empty() {
            *buffer = None;
            *count = 0;
            return;
        }

        *buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        *count = instances.len() as u32;
    }

    fn sync_glyph_buffer(&mut self, device: &wgpu::Device, instances: &[GlyphInstance]) {
        if instances.is_empty() {
            self.glyph_buffer = None;
            self.glyph_count = 0;
            return;
        }

        self.glyph_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("timon.terminal.glyph-buffer"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        self.glyph_count = instances.len() as u32;
    }

    fn rebuild_text_layer(
        &mut self,
        primitive: &TerminalAtlasPrimitive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
    ) {
        let mut rects = Vec::new();
        let mut prepared_glyphs = Vec::new();
        let widget_width = bounds.width.max(1.0).round();
        let widget_height = bounds.height.max(1.0).round();

        rects.push(RectInstance {
            rect: [0.0, 0.0, widget_width, widget_height],
            color: color_to_f32(primitive.snapshot.background),
        });

        if let Ok(mut atlas) = primitive.atlas.lock() {
            for cell in &primitive.snapshot.cells {
                let selected = selection_contains(primitive.selection.as_ref(), cell);
                let cursor_on_cell = cursor_covers_cell(&primitive.snapshot, cell);
                let rect = physical_cell_rect(
                    cell.column,
                    cell.line,
                    cell.width.max(1),
                    primitive.font.metrics.cell_width,
                    primitive.font.metrics.cell_height,
                    primitive.scale_factor,
                );

                let background = if cursor_on_cell
                    && primitive.snapshot.show_cursor
                    && matches!(primitive.snapshot.cursor_shape, CursorShape::Block)
                {
                    primitive.snapshot.cursor_color
                } else if selected {
                    primitive.snapshot.selection_background
                } else {
                    cell.bg
                };

                if background != primitive.snapshot.background {
                    push_rect_instance(
                        &mut rects,
                        rect.0 as f32,
                        rect.1 as f32,
                        (rect.2 - rect.0) as f32,
                        (rect.3 - rect.1) as f32,
                        background,
                    );
                }

                if let Some(underline) = cell.underline {
                    append_underline_rects(
                        &mut rects,
                        underline,
                        rect.0 as f32,
                        rect.1 as f32,
                        (rect.2 - rect.0) as f32,
                        (rect.3 - rect.1) as f32,
                        cell.underline_color,
                    );
                }

                let Some((glyph, atlas_width, atlas_height)) = rasterized_glyph_for_cell_in_atlas(
                    &mut atlas,
                    &primitive.font,
                    primitive.scale_factor,
                    cell,
                ) else {
                    continue;
                };

                prepared_glyphs.push(PreparedGlyph {
                    x: (rect.0 + glyph.offset_x) as f32,
                    y: (rect.1 + glyph.offset_y) as f32,
                    width: glyph.width as f32,
                    height: glyph.height as f32,
                    page_index: glyph.page_index,
                    atlas_x: glyph.atlas_x,
                    atlas_y: glyph.atlas_y,
                    atlas_width,
                    atlas_height,
                    color: if cursor_on_cell
                        && primitive.snapshot.show_cursor
                        && matches!(primitive.snapshot.cursor_shape, CursorShape::Block)
                    {
                        primitive.snapshot.cursor_text
                    } else if selected {
                        primitive.snapshot.selection_foreground
                    } else {
                        cell.fg
                    },
                });
            }

            self.sync_atlas_texture(device, queue, &atlas);
        }

        let mut glyph_instances = Vec::with_capacity(prepared_glyphs.len());
        for glyph in prepared_glyphs {
            glyph_instances.push(GlyphInstance {
                rect: [glyph.x, glyph.y, glyph.width, glyph.height],
                uv_rect: [
                    glyph.atlas_x as f32 / glyph.atlas_width as f32,
                    glyph.atlas_y as f32 / glyph.atlas_height as f32,
                    (glyph.atlas_x + glyph.width as u32) as f32 / glyph.atlas_width as f32,
                    (glyph.atlas_y + glyph.height as u32) as f32 / glyph.atlas_height as f32,
                ],
                color: color_to_f32(glyph.color),
                extras: [glyph.page_index as f32, 0.0, 0.0, 0.0],
            });
        }

        Self::sync_rect_buffer(
            &mut self.text_rect_buffer,
            &mut self.text_rect_count,
            device,
            &rects,
            "timon.terminal.text-rect-buffer",
        );
        self.sync_glyph_buffer(device, &glyph_instances);
    }

    fn rebuild_overlay_layer(&mut self, primitive: &TerminalAtlasPrimitive, device: &wgpu::Device) {
        let mut rects = Vec::new();

        if primitive.snapshot.show_cursor
            && !matches!(primitive.snapshot.cursor_shape, CursorShape::Block)
        {
            let rect = cursor_visual_rect(
                primitive.snapshot.cursor_column,
                primitive.snapshot.cursor_line,
                primitive.snapshot.cursor_width.max(1),
                primitive.font.metrics.cell_width,
                primitive.font.metrics.cell_height,
                primitive.font.size,
                primitive.scale_factor,
            );
            append_cursor_rects(
                &mut rects,
                primitive.snapshot.cursor_shape,
                rect.0 as f32,
                rect.1 as f32,
                (rect.2 - rect.0) as f32,
                (rect.3 - rect.1) as f32,
                primitive.snapshot.cursor_color,
            );
        }

        Self::sync_rect_buffer(
            &mut self.overlay_rect_buffer,
            &mut self.overlay_rect_count,
            device,
            &rects,
            "timon.terminal.overlay-rect-buffer",
        );
    }
}

impl ShaderPipeline for TerminalAtlasPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let mut pipeline = Self::default();
        pipeline.ensure_resources(device, format);
        pipeline
    }
}

impl ShaderPrimitive for TerminalAtlasPrimitive {
    type Pipeline = TerminalAtlasPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        pipeline.sync_viewport(queue, [bounds.width.max(1.0), bounds.height.max(1.0)]);

        let text_key = terminal_surface_cache_key(
            &self.snapshot,
            &self.font,
            self.scale_factor,
            self.selection.as_ref(),
            bounds.size(),
        );
        if pipeline.text_cache_key != Some(text_key) {
            pipeline.rebuild_text_layer(self, device, queue, bounds);
            pipeline.text_cache_key = Some(text_key);
        }

        let overlay_key =
            terminal_overlay_cache_key(&self.snapshot, self.selection.as_ref(), bounds.size());
        if pipeline.overlay_cache_key != Some(overlay_key) {
            pipeline.rebuild_overlay_layer(self, device);
            pipeline.overlay_cache_key = Some(overlay_key);
        }
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(viewport_bind_group) = pipeline.viewport_bind_group.as_ref() else {
            return true;
        };
        let Some(rect_pipeline) = pipeline.rect_pipeline.as_ref() else {
            return true;
        };

        if let Some(buffer) = pipeline.text_rect_buffer.as_ref() {
            render_pass.set_pipeline(rect_pipeline);
            render_pass.set_bind_group(0, viewport_bind_group, &[]);
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.draw(0..4, 0..pipeline.text_rect_count);
        }

        if let (Some(glyph_pipeline), Some(glyph_buffer), Some(atlas_bind_group)) = (
            pipeline.glyph_pipeline.as_ref(),
            pipeline.glyph_buffer.as_ref(),
            pipeline.atlas_bind_group.as_ref(),
        ) {
            render_pass.set_pipeline(glyph_pipeline);
            render_pass.set_bind_group(0, viewport_bind_group, &[]);
            render_pass.set_bind_group(1, atlas_bind_group, &[]);
            render_pass.set_vertex_buffer(0, glyph_buffer.slice(..));
            render_pass.draw(0..4, 0..pipeline.glyph_count);
        }

        if let Some(buffer) = pipeline.overlay_rect_buffer.as_ref() {
            render_pass.set_pipeline(rect_pipeline);
            render_pass.set_bind_group(0, viewport_bind_group, &[]);
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.draw(0..4, 0..pipeline.overlay_rect_count);
        }

        true
    }
}

impl TerminalTheme {
    pub fn from_settings(colors: &TerminalColors) -> Self {
        let fallback = TerminalColors::atom_one_light();
        let normal = colors.normal.as_array();
        let bright = colors.bright.as_array();
        let fallback_normal = fallback.normal.as_array();
        let fallback_bright = fallback.bright.as_array();

        let mut ansi = [Color::BLACK; 16];
        for (index, slot) in ansi.iter_mut().enumerate() {
            let (value, fallback_value) = if index < 8 {
                (&normal[index], &fallback_normal[index])
            } else {
                (&bright[index - 8], &fallback_bright[index - 8])
            };
            *slot =
                parse_hex_color(value).unwrap_or_else(|| parse_hex_color(fallback_value).unwrap());
        }

        Self {
            background: parse_hex_color(&colors.primary.background)
                .unwrap_or_else(|| parse_hex_color(&fallback.primary.background).unwrap()),
            foreground: parse_hex_color(&colors.primary.foreground)
                .unwrap_or_else(|| parse_hex_color(&fallback.primary.foreground).unwrap()),
            cursor_color: parse_hex_color(&colors.cursor.cursor)
                .unwrap_or_else(|| parse_hex_color(&fallback.cursor.cursor).unwrap()),
            cursor_text: parse_hex_color(&colors.cursor.text)
                .unwrap_or_else(|| parse_hex_color(&fallback.cursor.text).unwrap()),
            selection_background: parse_hex_color(&colors.selection.background)
                .unwrap_or_else(|| parse_hex_color(&fallback.selection.background).unwrap()),
            selection_foreground: parse_hex_color(&colors.selection.text)
                .unwrap_or_else(|| parse_hex_color(&fallback.selection.text).unwrap()),
            ansi,
        }
    }
}

impl TerminalFont {
    pub fn from_settings(font: &FontSettings) -> Self {
        let size = font.size.max(10.0);
        let line_height = font.line_height.max(1.0);
        let family_name =
            canonical_terminal_font_name(&font.family).unwrap_or_else(|| "monospace".into());
        let font_face = resolve_terminal_font(&family_name);

        Self {
            size,
            line_height,
            thicken: if font.thicken { 0.6 } else { 0.0 },
            metrics: measure_terminal_metrics(font_face, size, line_height),
            family_name,
        }
    }

    pub fn iced_font(&self) -> iced::Font {
        resolve_terminal_font(&self.family_name)
    }
}

const PREWARM_ASCII_GLYPHS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789`-=[]\\\\;',./~!@#$%^&*()_+{}|:\\\"<>?";
const PREWARM_EXTRA_GLYPHS: &[&str] = &[
    "~", "│", "─", "└", "┘", "┌", "┐", "├", "┤", "┬", "┴", "┼", "╭", "╮", "╯", "╰", "·", "•",
];

pub fn prewarm_glyph_atlas(atlas: &Arc<Mutex<GlyphAtlas>>, font: &TerminalFont, scale_factor: f32) {
    let seed_cell = |text: String, bold: bool| TerminalCell {
        text,
        fg: Color::BLACK,
        bg: Color::TRANSPARENT,
        underline: None,
        underline_color: Color::BLACK,
        width: 1,
        bold,
        italic: false,
        dim: false,
        hidden: false,
        line: 0,
        column: 0,
    };

    for glyph in PREWARM_ASCII_GLYPHS.chars().map(|ch| ch.to_string()).chain(
        PREWARM_EXTRA_GLYPHS
            .iter()
            .map(|glyph| (*glyph).to_string()),
    ) {
        let _ =
            rasterized_glyph_for_cell(atlas, font, scale_factor, &seed_cell(glyph.clone(), false));
        let _ = rasterized_glyph_for_cell(atlas, font, scale_factor, &seed_cell(glyph, true));
    }
}

impl GlyphAtlas {
    pub fn new() -> Self {
        Self {
            glyphs: HashMap::new(),
            pages: Vec::new(),
            swash_cache: graphics_text::cosmic_text::SwashCache::new(),
        }
    }

    fn page(&self, index: usize) -> Option<&MaskAtlasPage> {
        self.pages.get(index)
    }

    fn insert_mask(
        &mut self,
        key: GlyphKey,
        width: u32,
        height: u32,
        offset_x: i32,
        offset_y: i32,
        pixels: Vec<u8>,
    ) -> Option<RasterizedGlyph> {
        let (page_index, atlas_x, atlas_y) = self.allocate(width, height)?;
        let page = self.pages.get_mut(page_index)?;

        for row in 0..height as usize {
            let src_start = row * width as usize;
            let src_end = src_start + width as usize;
            let dst_start = ((atlas_y as usize + row) * page.width as usize) + atlas_x as usize;
            let dst_end = dst_start + width as usize;

            page.pixels[dst_start..dst_end].copy_from_slice(&pixels[src_start..src_end]);
        }
        page.version = page.version.saturating_add(1);

        let glyph = RasterizedGlyph {
            page_index,
            atlas_x,
            atlas_y,
            width,
            height,
            offset_x,
            offset_y,
        };

        self.glyphs.insert(key, glyph.clone());
        Some(glyph)
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<(usize, u32, u32)> {
        for (index, page) in self.pages.iter_mut().enumerate() {
            if let Some(position) = page.allocate(width, height) {
                return Some((index, position.0, position.1));
            }
        }

        let mut page = MaskAtlasPage::new(2048, 2048);
        let position = page.allocate(width, height)?;
        self.pages.push(page);
        Some((self.pages.len() - 1, position.0, position.1))
    }
}

impl MaskAtlasPage {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
            version: 0,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width == 0 || height == 0 || width > self.width || height > self.height {
            return None;
        }

        if self.cursor_x + width > self.width {
            self.cursor_x = 0;
            self.cursor_y = self.cursor_y.saturating_add(self.row_height);
            self.row_height = 0;
        }

        if self.cursor_y + height > self.height {
            return None;
        }

        let position = (self.cursor_x, self.cursor_y);
        self.cursor_x = self.cursor_x.saturating_add(width);
        self.row_height = self.row_height.max(height);
        Some(position)
    }
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

        let fg = resolve_color(indexed.cell.fg, renderable.colors, theme);
        let fg = if flags.contains(TermCellFlags::DIM) {
            fg.scale_alpha(0.8)
        } else {
            fg
        };
        let underline_color = indexed
            .cell
            .underline_color()
            .map(|color| resolve_color(color, renderable.colors, theme))
            .unwrap_or(fg);

        cells.push(TerminalCell {
            text,
            fg,
            bg: resolve_color(indexed.cell.bg, renderable.colors, theme),
            underline: underline_style(flags),
            underline_color,
            width: if flags.contains(TermCellFlags::WIDE_CHAR) {
                2
            } else {
                1
            },
            bold: flags.intersects(TermCellFlags::BOLD | TermCellFlags::DIM_BOLD),
            italic: flags.contains(TermCellFlags::ITALIC),
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

fn resolve_color(color: AnsiColor, colors: &Colors, theme: &TerminalTheme) -> Color {
    match color {
        AnsiColor::Named(named) => {
            color_from_rgb(colors[named].unwrap_or_else(|| fallback_named_color(named, theme)))
        }
        AnsiColor::Spec(rgb) => color_from_rgb(rgb),
        AnsiColor::Indexed(index) => color_from_rgb(
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

fn color_from_rgb(rgb: Rgb) -> Color {
    Color::from_rgb8(rgb.r, rgb.g, rgb.b)
}

fn to_rgb(color: Color) -> Rgb {
    let [r, g, b, _] = color.into_rgba8();
    Rgb { r, g, b }
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let value = value.trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&value[0..2], 16).ok()?;
    let g = u8::from_str_radix(&value[2..4], 16).ok()?;
    let b = u8::from_str_radix(&value[4..6], 16).ok()?;
    Some(Color::from_rgb8(r, g, b))
}

fn underline_style(flags: TermCellFlags) -> Option<UnderlineStyle> {
    if flags.contains(TermCellFlags::DOUBLE_UNDERLINE) {
        Some(UnderlineStyle::Double)
    } else if flags.contains(TermCellFlags::UNDERCURL) {
        Some(UnderlineStyle::Curly)
    } else if flags.contains(TermCellFlags::DOTTED_UNDERLINE) {
        Some(UnderlineStyle::Dotted)
    } else if flags.contains(TermCellFlags::DASHED_UNDERLINE) {
        Some(UnderlineStyle::Dashed)
    } else if flags.contains(TermCellFlags::UNDERLINE) {
        Some(UnderlineStyle::Single)
    } else {
        None
    }
}

fn font_for_glyph(key: &GlyphKey) -> iced::Font {
    let resolved = resolved_font_from_family_name(&key.family_name);

    iced::Font {
        family: resolved.family,
        weight: if key.bold {
            FontWeight::Bold
        } else {
            resolved.weight
        },
        style: if key.italic {
            FontStyle::Italic
        } else {
            resolved.style
        },
        stretch: resolved.stretch,
    }
}

fn resolved_font_from_family_name(family_name: &str) -> iced::Font {
    let normalized = family_name.trim().to_ascii_lowercase();

    let family = match normalized.as_str() {
        "mono" | "monospace" | "system-monospace" => iced::font::Family::Monospace,
        "sans" | "sans-serif" | "system-ui" => iced::font::Family::SansSerif,
        "serif" => iced::font::Family::Serif,
        "cursive" => iced::font::Family::Cursive,
        "fantasy" => iced::font::Family::Fantasy,
        _ => {
            let owned = family_name.trim().to_string().into_boxed_str();
            return iced::Font {
                family: iced::font::Family::Name(Box::leak(owned)),
                ..Default::default()
            };
        }
    };

    iced::Font {
        family,
        ..Default::default()
    }
}

fn rasterized_glyph_for_cell(
    atlas: &Arc<Mutex<GlyphAtlas>>,
    font: &TerminalFont,
    scale_factor: f32,
    cell: &TerminalCell,
) -> Option<RasterizedGlyph> {
    let mut atlas = atlas.lock().ok()?;
    rasterized_glyph_for_cell_in_atlas(&mut atlas, font, scale_factor, cell)
        .map(|(glyph, _, _)| glyph)
}

fn rasterized_glyph_for_cell_in_atlas(
    atlas: &mut GlyphAtlas,
    font: &TerminalFont,
    scale_factor: f32,
    cell: &TerminalCell,
) -> Option<(RasterizedGlyph, u32, u32)> {
    let key = glyph_key_for_cell(font, scale_factor, cell)?;

    if let Some(glyph) = atlas.glyphs.get(&key) {
        let glyph = glyph.clone();
        let page = atlas.page(glyph.page_index)?;
        return Some((glyph, page.width, page.height));
    }

    let (width, height, offset_x, offset_y, pixels) =
        rasterize_glyph(font, scale_factor, &key, &mut atlas.swash_cache)?;

    let glyph = atlas.insert_mask(key, width, height, offset_x, offset_y, pixels)?;
    let page = atlas.page(glyph.page_index)?;
    Some((glyph, page.width, page.height))
}

fn glyph_key_for_cell(
    font: &TerminalFont,
    scale_factor: f32,
    cell: &TerminalCell,
) -> Option<GlyphKey> {
    if cell.hidden || cell.text.trim().is_empty() {
        return None;
    }

    let family_name = glyph_family_name(&font.family_name, &cell.text);

    Some(GlyphKey {
        text: cell.text.clone(),
        family_name,
        cell_columns: cell.width.min(u8::MAX as usize) as u8,
        font_size_bits: font.size.to_bits(),
        line_height_bits: font.line_height.to_bits(),
        thicken_bits: font.thicken.to_bits(),
        scale_factor_bits: scale_factor.to_bits(),
        bold: cell.bold,
        italic: cell.italic,
        dim: cell.dim,
    })
}

fn rasterize_glyph(
    font: &TerminalFont,
    scale_factor: f32,
    key: &GlyphKey,
    swash_cache: &mut graphics_text::cosmic_text::SwashCache,
) -> Option<(u32, u32, i32, i32, Vec<u8>)> {
    const PAD_X: i32 = 2;
    const PAD_Y: i32 = 2;

    let physical_scale = scale_factor.max(1.0);
    let physical_font_size = font.size * physical_scale;
    let physical_line_height = font.metrics.cell_height * physical_scale;
    let physical_cell_width =
        (font.metrics.cell_width * f32::from(key.cell_columns) * physical_scale).ceil() as i32;
    let physical_cell_height = physical_line_height.ceil() as i32;
    let mut font_system = graphics_text::font_system().write().ok()?;
    let mut buffer = graphics_text::cosmic_text::Buffer::new(
        font_system.raw(),
        graphics_text::cosmic_text::Metrics::new(physical_font_size, physical_line_height),
    );

    buffer.set_wrap(font_system.raw(), graphics_text::cosmic_text::Wrap::None);
    buffer.set_size(font_system.raw(), None, None);

    let font_face = font_for_glyph(key);
    buffer.set_text(
        font_system.raw(),
        &key.text,
        &graphics_text::to_attributes(font_face),
        graphics_text::cosmic_text::Shaping::Advanced,
        None,
    );
    let color = graphics_text::cosmic_text::Color::rgb(255, 255, 255);
    let width = (physical_cell_width + PAD_X * 2).max(1) as u32;
    let height = (physical_cell_height + PAD_Y * 2).max(1) as u32;
    let mut pixels = vec![0u8; (width * height) as usize];
    let mut has_ink = false;

    buffer.draw(
        font_system.raw(),
        swash_cache,
        color,
        |px, py, _, _, color| {
            let [r, g, b, a] = color.as_rgba();
            if a == 0 {
                return;
            }

            let x = px + PAD_X;
            let y = py + PAD_Y;

            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                return;
            }

            let coverage = mask_coverage_from_rgba(r, g, b, a);
            if coverage == 0 {
                return;
            }

            let coverage = apply_font_thicken(coverage, f32::from_bits(key.thicken_bits));
            if coverage == 0 {
                return;
            }

            let index = (y as u32 * width + x as u32) as usize;
            pixels[index] = pixels[index].max(coverage);
            has_ink = true;
        },
    );

    if !has_ink {
        return None;
    }

    Some((width, height, -PAD_X, -PAD_Y, pixels))
}

fn mask_coverage_from_rgba(_r: u8, _g: u8, _b: u8, a: u8) -> u8 {
    // For regular monochrome glyphs, cosmic-text emits `Content::Mask` through
    // `with_pixels`, which stores the actual coverage in the alpha channel and
    // just copies the requested base RGB.
    a
}

fn terminal_surface_cache_key(
    snapshot: &TerminalSnapshot,
    font: &TerminalFont,
    scale_factor: f32,
    selection: Option<&TerminalSelection>,
    size: Size,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    size.width.to_bits().hash(&mut hasher);
    size.height.to_bits().hash(&mut hasher);
    scale_factor.to_bits().hash(&mut hasher);
    font.size.to_bits().hash(&mut hasher);
    font.line_height.to_bits().hash(&mut hasher);
    font.thicken.to_bits().hash(&mut hasher);
    font.family_name.hash(&mut hasher);

    snapshot.background.into_rgba8().hash(&mut hasher);
    snapshot.cursor_line.hash(&mut hasher);
    snapshot.cursor_column.hash(&mut hasher);
    snapshot.cursor_width.hash(&mut hasher);
    std::mem::discriminant(&snapshot.cursor_shape).hash(&mut hasher);
    snapshot.show_cursor.hash(&mut hasher);
    snapshot.cursor_blinking.hash(&mut hasher);
    snapshot.cursor_color.into_rgba8().hash(&mut hasher);
    snapshot.cursor_text.into_rgba8().hash(&mut hasher);
    snapshot.selection_background.into_rgba8().hash(&mut hasher);
    snapshot.selection_foreground.into_rgba8().hash(&mut hasher);

    snapshot.cells.len().hash(&mut hasher);
    for cell in &snapshot.cells {
        cell.text.hash(&mut hasher);
        cell.fg.into_rgba8().hash(&mut hasher);
        cell.bg.into_rgba8().hash(&mut hasher);
        cell.underline_color.into_rgba8().hash(&mut hasher);
        cell.width.hash(&mut hasher);
        cell.bold.hash(&mut hasher);
        cell.italic.hash(&mut hasher);
        cell.dim.hash(&mut hasher);
        cell.hidden.hash(&mut hasher);
        cell.line.hash(&mut hasher);
        cell.column.hash(&mut hasher);
        match cell.underline {
            Some(style) => std::mem::discriminant(&style).hash(&mut hasher),
            None => 0u8.hash(&mut hasher),
        }
    }

    if let Some(selection) = selection {
        selection.start.line.hash(&mut hasher);
        selection.start.column.hash(&mut hasher);
        selection.end.line.hash(&mut hasher);
        selection.end.column.hash(&mut hasher);
    } else {
        0usize.hash(&mut hasher);
    }

    hasher.finish()
}

fn terminal_overlay_cache_key(
    snapshot: &TerminalSnapshot,
    selection: Option<&TerminalSelection>,
    size: Size,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    size.width.to_bits().hash(&mut hasher);
    size.height.to_bits().hash(&mut hasher);
    snapshot.cursor_line.hash(&mut hasher);
    snapshot.cursor_column.hash(&mut hasher);
    snapshot.cursor_width.hash(&mut hasher);
    std::mem::discriminant(&snapshot.cursor_shape).hash(&mut hasher);
    snapshot.show_cursor.hash(&mut hasher);
    snapshot.cursor_blinking.hash(&mut hasher);
    snapshot.cursor_color.into_rgba8().hash(&mut hasher);

    if let Some(selection) = selection {
        selection.start.line.hash(&mut hasher);
        selection.start.column.hash(&mut hasher);
        selection.end.line.hash(&mut hasher);
        selection.end.column.hash(&mut hasher);
    } else {
        0usize.hash(&mut hasher);
    }

    hasher.finish()
}

fn physical_cell_rect(
    column: usize,
    line: usize,
    cell_columns: usize,
    cell_width: f32,
    cell_height: f32,
    scale: f32,
) -> (i32, i32, i32, i32) {
    let x0 = ((column as f32) * cell_width * scale).round() as i32;
    let y0 = ((line as f32) * cell_height * scale).round() as i32;
    let x1 = (((column + cell_columns) as f32) * cell_width * scale).round() as i32;
    let y1 = (((line + 1) as f32) * cell_height * scale).round() as i32;
    (x0, y0, x1.max(x0 + 1), y1.max(y0 + 1))
}

fn cursor_visual_rect(
    column: usize,
    line: usize,
    cell_columns: usize,
    cell_width: f32,
    cell_height: f32,
    font_size: f32,
    scale: f32,
) -> (i32, i32, i32, i32) {
    let (x0, y0, x1, y1) =
        physical_cell_rect(column, line, cell_columns, cell_width, cell_height, scale);
    let full_height = (y1 - y0).max(1);
    let target_height = (font_size * scale).round() as i32;
    let visual_height = target_height.clamp(1, full_height);
    let inset = ((full_height - visual_height) / 2).max(0);

    (
        x0,
        y0 + inset,
        x1,
        (y0 + inset + visual_height).max(y0 + inset + 1),
    )
}

fn cursor_covers_cell(snapshot: &TerminalSnapshot, cell: &TerminalCell) -> bool {
    if cell.line != snapshot.cursor_line {
        return false;
    }

    let cursor_start = snapshot.cursor_column;
    let cursor_end = snapshot.cursor_column + snapshot.cursor_width.max(1) - 1;
    let cell_start = cell.column;
    let cell_end = cell.column + cell.width.max(1) - 1;

    cell_end >= cursor_start && cell_start <= cursor_end
}

fn apply_font_thicken(coverage: u8, thicken: f32) -> u8 {
    const COVERAGE_CUTOFF: f32 = 0.012;

    let normalized = f32::from(coverage) / 255.0;
    if normalized <= COVERAGE_CUTOFF {
        return 0;
    }

    let gamma = (1.0 - thicken * 0.22).clamp(0.72, 1.18);
    let gain = (1.0 + thicken * 0.16).clamp(0.82, 1.35);

    let adjusted = (normalized.powf(gamma) * gain).clamp(0.0, 1.0);
    (adjusted * 255.0).round() as u8
}

fn color_to_f32(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn push_rect_instance(
    rects: &mut Vec<RectInstance>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    rects.push(RectInstance {
        rect: [x, y, width, height],
        color: color_to_f32(color),
    });
}

fn append_underline_rects(
    rects: &mut Vec<RectInstance>,
    underline: UnderlineStyle,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
) {
    let baseline = y + height - 2.0;
    match underline {
        UnderlineStyle::Single => push_rect_instance(rects, x, baseline, width, 1.0, color),
        UnderlineStyle::Double => {
            push_rect_instance(rects, x, baseline - 2.0, width, 1.0, color);
            push_rect_instance(rects, x, baseline, width, 1.0, color);
        }
        UnderlineStyle::Dotted => {
            let mut offset = 0.0;
            while offset < width {
                push_rect_instance(rects, x + offset, baseline, 1.0, 1.0, color);
                offset += 3.0;
            }
        }
        UnderlineStyle::Dashed => {
            let mut offset = 0.0;
            while offset < width {
                push_rect_instance(
                    rects,
                    x + offset,
                    baseline,
                    (width - offset).min(4.0),
                    1.0,
                    color,
                );
                offset += 6.0;
            }
        }
        UnderlineStyle::Curly => {
            let mut offset = 0.0;
            let mut up = false;
            while offset < width {
                push_rect_instance(
                    rects,
                    x + offset,
                    if up { baseline - 1.0 } else { baseline },
                    (width - offset).min(2.0),
                    1.0,
                    color,
                );
                offset += 2.0;
                up = !up;
            }
        }
    }
}

fn append_cursor_rects(
    rects: &mut Vec<RectInstance>,
    cursor_shape: CursorShape,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
) {
    match cursor_shape {
        CursorShape::Block => push_rect_instance(rects, x, y, width, height, color),
        CursorShape::HollowBlock => {
            push_rect_instance(rects, x, y, width, 1.0, color);
            push_rect_instance(rects, x, y + height - 1.0, width, 1.0, color);
            push_rect_instance(rects, x, y, 1.0, height, color);
            push_rect_instance(rects, x + width - 1.0, y, 1.0, height, color);
        }
        CursorShape::Underline => push_rect_instance(rects, x, y + height - 2.0, width, 2.0, color),
        CursorShape::Beam => push_rect_instance(rects, x, y, 2.0, height, color),
        CursorShape::Hidden => {}
    }
}

const STABLE_CJK_FALLBACKS: &[&str] = &[
    "PingFang SC",
    "Hiragino Sans GB",
    "Songti SC",
    "STHeiti",
    "Heiti SC",
    "Noto Sans CJK SC",
];

fn selection_contains(selection: Option<&TerminalSelection>, cell: &TerminalCell) -> bool {
    let Some(selection) = selection else {
        return false;
    };

    let cell_start = (cell.line, cell.column);
    let cell_end = (cell.line, cell.column + cell.width.saturating_sub(1));
    let selection_start = (selection.start.line, selection.start.column);
    let selection_end = (selection.end.line, selection.end.column);

    cell_end >= selection_start && cell_start <= selection_end
}

fn resolve_terminal_font(family_name: &str) -> iced::Font {
    let trimmed = family_name.trim();

    if trimmed.is_empty() {
        return iced::Font::MONOSPACE;
    }

    let normalized = trimmed.to_ascii_lowercase();

    let generic = match normalized.as_str() {
        "mono" | "monospace" | "system-monospace" => Some(iced::font::Family::Monospace),
        "sans" | "sans-serif" | "system-ui" => Some(iced::font::Family::SansSerif),
        "serif" => Some(iced::font::Family::Serif),
        "cursive" => Some(iced::font::Family::Cursive),
        "fantasy" => Some(iced::font::Family::Fantasy),
        _ => None,
    };

    if let Some(family) = generic {
        return iced::Font {
            family,
            ..Default::default()
        };
    }

    let canonical = resolve_canonical_family_name(trimmed)
        .unwrap_or_else(|| trimmed.to_string())
        .into_boxed_str();

    iced::Font {
        family: iced::font::Family::Name(Box::leak(canonical)),
        ..Default::default()
    }
}

pub fn canonical_terminal_font_name(requested: &str) -> Option<String> {
    let trimmed = requested.trim();

    if trimmed.is_empty() {
        return Some("monospace".into());
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "mono" | "monospace" | "system-monospace" => return Some("monospace".into()),
        "sans" | "sans-serif" | "system-ui" => return Some("sans-serif".into()),
        "serif" => return Some("serif".into()),
        "cursive" => return Some("cursive".into()),
        "fantasy" => return Some("fantasy".into()),
        _ => {}
    }

    resolve_canonical_family_name(trimmed)
}

pub fn available_terminal_fonts() -> Vec<String> {
    let mut families = BTreeSet::new();
    families.insert("monospace".to_string());

    if let Ok(mut font_system) = graphics_text::font_system().write() {
        let database = font_system.raw().db();

        for face in database.faces() {
            if !face.monospaced {
                continue;
            }

            for family in &face.families {
                families.insert(family.0.clone());
            }
        }
    }

    let mut fonts = families.into_iter().collect::<Vec<_>>();
    fonts.sort_by(|a, b| {
        if a == "monospace" {
            return std::cmp::Ordering::Less;
        }
        if b == "monospace" {
            return std::cmp::Ordering::Greater;
        }

        a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
    });
    fonts
}

fn resolve_canonical_family_name(requested: &str) -> Option<String> {
    let mut font_system = graphics_text::font_system().write().ok()?;
    let database = font_system.raw().db();

    database.faces().find_map(|face| {
        face.families.iter().find_map(|family| {
            family
                .0
                .eq_ignore_ascii_case(requested)
                .then(|| family.0.clone())
        })
    })
}

fn glyph_family_name(base_family_name: &str, text: &str) -> String {
    if !contains_cjk(text) || family_prefers_own_cjk_glyphs(base_family_name) {
        return base_family_name.to_string();
    }

    stable_cjk_fallback_family().unwrap_or_else(|| base_family_name.to_string())
}

fn stable_cjk_fallback_family() -> Option<String> {
    static FALLBACK: OnceLock<Option<String>> = OnceLock::new();

    FALLBACK
        .get_or_init(|| {
            STABLE_CJK_FALLBACKS
                .iter()
                .find_map(|family| resolve_canonical_family_name(family))
        })
        .clone()
}

fn family_prefers_own_cjk_glyphs(family_name: &str) -> bool {
    let normalized = family_name.trim().to_ascii_lowercase();

    [
        "pingfang",
        "hiragino",
        "songti",
        "heiti",
        "stheiti",
        "noto sans cjk",
        "source han",
        "sarasa",
        "wenkai",
        "mono sc",
        "mono tc",
        "mono hc",
        "mono jp",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_char)
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2E80..=0x2EFF
            | 0x2F00..=0x2FDF
            | 0x3040..=0x30FF
            | 0x3100..=0x312F
            | 0x3130..=0x318F
            | 0x31A0..=0x31BF
            | 0x31C0..=0x31EF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x30000..=0x3134F
    )
}

fn measure_terminal_metrics(
    font: iced::Font,
    size: f32,
    line_height_factor: f32,
) -> TerminalMetrics {
    const SAMPLE: &str = "MMMMMMMMMM";

    let fallback = TerminalMetrics {
        cell_width: (size * 0.62).max(1.0),
        cell_height: (size * line_height_factor).max(1.0),
    };

    let mut font_system = match graphics_text::font_system().write() {
        Ok(font_system) => font_system,
        Err(_) => return fallback,
    };

    let mut buffer = graphics_text::cosmic_text::Buffer::new(
        font_system.raw(),
        graphics_text::cosmic_text::Metrics::new(size, fallback.cell_height),
    );

    buffer.set_wrap(font_system.raw(), graphics_text::cosmic_text::Wrap::None);
    buffer.set_size(font_system.raw(), None, None);
    buffer.set_text(
        font_system.raw(),
        SAMPLE,
        &graphics_text::to_attributes(font),
        graphics_text::cosmic_text::Shaping::Advanced,
        None,
    );

    let Some(run) = buffer.layout_runs().next() else {
        return fallback;
    };

    let sample_len = SAMPLE.chars().count() as f32;
    let cell_width = (run.line_w / sample_len).max(fallback.cell_width);

    TerminalMetrics {
        cell_width: cell_width.max(1.0),
        cell_height: run.line_height.max(1.0),
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

fn cursor_key(app_cursor: bool, suffix: u8, shift: bool, alt: bool) -> Vec<u8> {
    if shift || alt {
        let modifier = match (shift, alt) {
            (true, true) => 4,
            (true, false) => 2,
            (false, true) => 3,
            (false, false) => 1,
        };
        vec![0x1b, b'[', b'1', b';', b'0' + modifier, suffix]
    } else if app_cursor {
        vec![0x1b, b'O', suffix]
    } else {
        vec![0x1b, b'[', suffix]
    }
}

fn ctrl_sequence(key: &Key) -> Option<Vec<u8>> {
    match key.as_ref() {
        Key::Character("a") | Key::Character("A") => Some(vec![0x01]),
        Key::Character("b") | Key::Character("B") => Some(vec![0x02]),
        Key::Character("c") | Key::Character("C") => Some(vec![0x03]),
        Key::Character("d") | Key::Character("D") => Some(vec![0x04]),
        Key::Character("e") | Key::Character("E") => Some(vec![0x05]),
        Key::Character("f") | Key::Character("F") => Some(vec![0x06]),
        Key::Character("g") | Key::Character("G") => Some(vec![0x07]),
        Key::Character("h") | Key::Character("H") => Some(vec![0x08]),
        Key::Character("k") | Key::Character("K") => Some(vec![0x0b]),
        Key::Character("l") | Key::Character("L") => Some(vec![0x0c]),
        Key::Character("n") | Key::Character("N") => Some(vec![0x0e]),
        Key::Character("p") | Key::Character("P") => Some(vec![0x10]),
        Key::Character("t") | Key::Character("T") => Some(vec![0x14]),
        Key::Character("u") | Key::Character("U") => Some(vec![0x15]),
        Key::Character("w") | Key::Character("W") => Some(vec![0x17]),
        Key::Character("z") | Key::Character("Z") => Some(vec![0x1a]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::TerminalSettings;

    fn snapshot_line_text(snapshot: &TerminalSnapshot, line: usize) -> String {
        let mut cells = snapshot
            .cells
            .iter()
            .filter(|cell| cell.line == line)
            .collect::<Vec<_>>();
        cells.sort_by_key(|cell| cell.column);
        cells.into_iter().map(|cell| cell.text.as_str()).collect()
    }

    #[test]
    fn rasterize_glyph_produces_non_empty_mask() {
        let settings = FontSettings::default();
        let font = TerminalFont::from_settings(&settings);
        let key = GlyphKey {
            text: "A".into(),
            family_name: font.family_name.clone(),
            cell_columns: 1,
            font_size_bits: font.size.to_bits(),
            line_height_bits: font.line_height.to_bits(),
            thicken_bits: font.thicken.to_bits(),
            scale_factor_bits: 1.0f32.to_bits(),
            bold: false,
            italic: false,
            dim: false,
        };

        let mut swash_cache = graphics_text::cosmic_text::SwashCache::new();
        let (_, _, _, _, pixels) =
            rasterize_glyph(&font, 1.0, &key, &mut swash_cache).expect("glyph should rasterize");

        assert!(pixels.iter().any(|coverage| *coverage > 0));
    }

    #[test]
    fn rasterized_glyph_is_inserted_into_atlas() {
        let settings = FontSettings::default();
        let font = TerminalFont::from_settings(&settings);
        let cell = TerminalCell {
            text: "A".into(),
            fg: Color::BLACK,
            bg: Color::WHITE,
            underline: None,
            underline_color: Color::BLACK,
            width: 1,
            bold: false,
            italic: false,
            dim: false,
            hidden: false,
            line: 0,
            column: 0,
        };
        let atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
        let glyph = rasterized_glyph_for_cell(&atlas, &font, 1.0, &cell)
            .expect("glyph should be rasterized and cached");
        let atlas = atlas.lock().expect("atlas should lock");
        let page = atlas
            .page(glyph.page_index)
            .expect("glyph page should exist in atlas");

        let has_non_zero_coverage = page.pixels.iter().any(|coverage| *coverage > 0);
        assert!(
            has_non_zero_coverage,
            "atlas page should contain glyph coverage"
        );
    }

    #[test]
    fn snapshot_scrollback_lines_stay_on_separate_rows() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(4, 2, &settings);

        terminal.push_local_line("1111");
        terminal.push_local_line("2222");
        terminal.push_local_line("3333");
        terminal.term.scroll_display(Scroll::Delta(2));

        let snapshot = terminal.snapshot(&theme);

        assert_eq!(snapshot_line_text(&snapshot, 0), "1111");
        assert_eq!(snapshot_line_text(&snapshot, 1), "2222");
    }

    #[test]
    fn viewport_scrolls_when_output_reaches_bottom() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(4, 2, &settings);

        terminal.feed(b"1111\r\n2222\r\n3333");

        let snapshot = terminal.snapshot(&theme);

        assert_eq!(snapshot_line_text(&snapshot, 0), "2222");
        assert_eq!(snapshot_line_text(&snapshot, 1), "3333");
    }

    #[test]
    fn alternate_screen_restores_primary_buffer() {
        let settings = TerminalSettings::default();
        let theme = TerminalTheme::from_settings(&settings.colors);
        let mut terminal = TerminalView::new(8, 2, &settings);

        terminal.feed(b"main");
        terminal.feed(b"\x1b[?1049h");
        terminal.feed(b"alt");
        terminal.feed(b"\x1b[?1049l");

        let snapshot = terminal.snapshot(&theme);

        assert!(!terminal.term.mode().contains(TermMode::ALT_SCREEN));
        assert_eq!(snapshot_line_text(&snapshot, 0).trim_end(), "main");
        assert!(!snapshot_line_text(&snapshot, 0).contains("alt"));
    }
}
