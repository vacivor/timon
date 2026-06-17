# Slint Migration Blueprint

This application is currently built around an `iced` runtime with three large UI-coupled areas:

- `src/app/view.rs`
- `src/app/update.rs`
- `src/app/state.rs`

It also has three important non-UI subsystems that we should preserve:

- `src/persistence.rs`: SQLite, settings, themes, known_hosts
- `src/session.rs`: local shell, SSH, SFTP, port forwarding
- `src/terminal.rs`: terminal model, input encoding, custom renderer

The safest migration path is not a one-shot rewrite. Instead, we should move to Slint in phases while keeping the current application runnable.

## Phase 1: Shared Core

Goal: extract data loading and app services so both `iced` and `slint` can consume the same backend.

Current progress:

- `src/workspace.rs` now owns `WorkspaceData::load(...)`
- `App::new` and `App::reload_data` consume `WorkspaceData` instead of querying SQLite directly

Next extractions in this phase:

- theme discovery and normalization
- connection/group/key/identity/forward CRUD service layer
- app-level commands for connect/disconnect/open drawer/context menu actions

## Phase 2: Slint Terminal Surface

Goal: move terminal rendering into a Slint-hosted surface first.

Current progress:

- `Cargo.toml` defines the optional `slint-ui` feature
- `Cargo.toml` now makes `slint-ui` the default feature, so normal `cargo run --bin Timon` and macOS packaging builds launch the Slint shell path
- `Cargo.toml` keeps the legacy iced app behind an explicit `iced-ui` feature; legacy validation should use `--no-default-features --features iced-ui`
- `Cargo.toml` keeps the legacy `bytemuck` dependency behind `iced-ui`, because it is only needed by the old wgpu glyph-atlas renderer and not by the Slint terminal path
- `Cargo.toml` defines the `TimonSlintTerminal` binary for terminal-first migration work
- `src/slint_terminal_app.rs` now owns the reusable Slint terminal entry point, while `src/slint_main.rs` is only the thin `TimonSlintTerminal` binary wrapper
- `src/main.rs` can route `Timon --features slint-ui -- --slint-terminal` into the same Slint terminal entry point and strips that internal mode argument before handing the rest of the CLI args to `src/slint_terminal_app.rs`
- `src/slint_terminal_core.rs` owns a Slint-specific terminal model bridge built on `alacritty_terminal`
- `src/slint_terminal.rs` renders terminal cells with Slint-native `Rectangle` and `Text` repeaters
- `src/slint_terminal_app.rs` starts a live local PTY session and refreshes Slint cell models from terminal output
- `src/slint_terminal.rs` captures keyboard input with modifier state and forwards it to the live session
- `src/slint_terminal.rs` captures terminal key input before Slint focus navigation so Tab/Shift+Tab and navigation keys stay available to shells, vim, and tmux
- `src/slint_terminal_core.rs` encodes terminal key input for text, Ctrl, Alt, Shift/Alt arrows, Home/End, PageUp/PageDown, Delete, Insert, and F1-F12
- `src/slint_terminal_core.rs` encodes xterm-style Shift/Alt/Control modifiers for cursor, navigation, delete/page, and function keys
- `TimonSlintTerminal` tracks Slint window size and forwards PTY resize events
- `TimonSlintTerminal` handles Slint pointer drag selection and wheel scrolling
- `TimonSlintTerminal` wires terminal-generated input events back to the live session, including alternate-screen wheel scrolling
- `TimonSlintTerminal` reports left-button mouse press, release, and drag events back to mouse-aware terminal applications
- `TimonSlintTerminal` reports focus in/out events back to terminal applications that enable focus tracking
- `TimonSlintTerminal` hides block/beam/underline cursor rendering while the terminal surface is not focused
- `TimonSlintTerminal` scrolls back to the live bottom and clears local selection when terminal input or paste is sent
- `TimonSlintTerminal` supports double-click word selection and triple-click terminal-token selection
- `TimonSlintTerminal` supports copying the current terminal selection and pasting clipboard text through terminal-aware newline/bracketed-paste encoding
- `src/slint_terminal_core.rs` has coverage for multiline bracketed paste encoding used by vim-style terminal applications
- `src/slint_terminal_core.rs` has real CJK wide-cell selection coverage so copying from either half of a wide cell preserves the full glyph and following columns
- `TimonSlintTerminal` drives block cursor blinking from Slint UI state without the old iced widget state
- `TimonSlintTerminal` renders beam and underline cursors with a Slint-native overlay rectangle
- `src/slint_terminal_core.rs` has parser-backed coverage for cursor width inside CJK wide cells before Slint renders block or underline cursors
- `TimonSlintTerminal` consumes terminal title/reset-title events and binds them to the Slint window title
- `TimonSlintTerminal` renders underline, double underline, dotted/dashed/curly underline approximations, and strikeout with Slint-native decoration rectangles
- `TimonSlintTerminal` applies reverse-video / inverse cell colors in the Slint terminal core
- `TimonSlintTerminal` forwards PTY `SessionEvent` output into the Slint event loop instead of relying on fixed-frame polling
- `TimonSlintTerminal` preserves wide-cell geometry when mapping terminal snapshots into Slint cell models
- `src/slint_terminal_core.rs` has parser-level coverage for CJK wide cells from real terminal input before they reach the Slint model layer
- `TimonSlintTerminal` isolates Slint window-to-terminal-grid sizing in a pure tested boundary, so native font measurement can replace the temporary metrics estimate without touching PTY resize dispatch
- `TimonSlintTerminal` reads native Slint `Text` metrics for terminal cell width and height, while keeping the size-based estimate only as a startup fallback if measurement is unavailable
- `TimonSlintTerminal` refreshes native Slint text metrics through the same periodic resize path, so late platform font resolution can still recompute the terminal grid and PTY size
- `TimonSlintTerminal` runs the same Slint layout synchronization during startup and timer ticks, so initial PTY sizing no longer waits for a separate first timer-only path when the window size is already available
- `TimonSlintTerminal` now uses one shared runtime refresh path for session-event callbacks and timer ticks, keeping terminal event draining, cursor blinking, and Slint rendering in sync
- `src/slint_terminal_core.rs` has parity tests for basic named key sequences, Tab/Shift+Tab, and application-cursor arrow mode
- `TimonSlintTerminal` sends an explicit session disconnect command when the Slint window exits, so the migration entry point does not rely only on channel drop timing to tear down PTY/SSH/serial sessions

This is not the final terminal integration yet. It proves the first usable migration boundary:

- terminal model remains reusable
- the Slint terminal no longer imports the old iced `src/terminal.rs` widget/shader module
- Slint can render terminal cells without the old glyph atlas / RGBA image path
- Slint terminal rendering uses native `Text` and `Rectangle` items; `glyph_atlas` remains only for the legacy iced terminal path until the main UI migration is complete
- pure Slint builds no longer have a direct `timon -> bytemuck` dependency from the legacy wgpu terminal renderer
- Slint can drive a real local shell through the existing `session.rs` backend
- Slint window resizing can resize both `TerminalView` and the backing PTY
- Slint alternate-screen wheel scrolling now writes terminal input back through the existing session command channel
- Slint mouse reporting now supports SGR and legacy left-button press/release/drag payloads while preserving local selection when mouse mode is disabled
- Slint focus reporting now emits `ESC[I` / `ESC[O` only when the terminal has enabled DEC focus mode
- Slint cursor rendering now follows terminal focus state, including block-cursor cell inversion
- Slint cursor geometry now has explicit parser-backed coverage for double-width CJK cells
- Slint input and paste now match the old terminal path by returning the display to the bottom before writing to the PTY
- Slint paste encoding now has explicit coverage for multiline bracketed-paste mode, including CRLF normalization inside the bracketed payload
- Slint keyboard handling now distinguishes macOS Command from terminal Control before sending bytes to the PTY
- Slint terminal focus no longer uses Tab for widget focus traversal; Tab is treated as terminal input
- Slint copy/paste no longer depends on iced clipboard APIs
- Slint selection behavior is now partly independent from iced, including word/token selection
- Slint selection extraction now has explicit parser-backed coverage for CJK wide cells at copy time
- Slint cursor visibility is now independent from iced redraw/input-method state
- Slint cursor shape rendering no longer depends on the iced/wgpu overlay renderer for beam and underline shapes
- Slint window title updates now come from the terminal core event stream instead of iced app state
- Slint text decoration rendering no longer depends on the iced/wgpu glyph atlas renderer
- Slint color resolution now handles inverse video before handing cells to the Slint renderer
- Slint cell-model conversion now has explicit coverage for double-width terminal cells, so CJK/wide glyph geometry is guarded at the renderer boundary
- Slint terminal core now verifies that parsed CJK input enters snapshots as a double-width cell instead of relying only on hand-built test cells
- Slint PTY output consumption is now event-loop driven; the timer remains only for resize, cursor blink, and other periodic UI work
- Slint window resizing now has tested grid conversion behavior, including scale factor handling, minimum grid clamping, and resize-command deduplication
- Slint terminal cell sizing now uses Slint's own text layout metrics instead of relying solely on the old fixed ratio estimate
- Slint terminal cell metrics are now kept in sync after startup, and a metrics change with the same window size can still trigger a correct PTY resize
- Slint startup layout synchronization now shares the same code path as periodic resize handling
- Slint runtime rendering refresh is centralized, so event-driven output and periodic cursor/layout updates no longer duplicate render decisions
- Slint terminal key handling now has explicit coverage for the old basic named-key behavior, including Tab/Shift+Tab and app-cursor arrow sequences
- Slint terminal window shutdown now has an explicit tested session-disconnect path
- Slint stable `FocusScope` does not expose input-method cursor rectangle updates; Slint's winit backend supports IME cursor areas through internal `TextInput` requests, so terminal IME positioning needs either an upstream/public API path or a custom backend/item bridge

Next work in this phase:

- port IME positioning once a stable Slint API or custom bridge for terminal cursor rectangles is available
- validate remaining advanced terminal key cases manually in shells, vim, tmux, and fish
- decide whether to replace the old iced `src/terminal.rs` with the Slint core or keep it only until the main UI has moved

## Phase 3: Session Bridge

Goal: bridge `session.rs` into a UI-neutral controller that can publish state changes to either frontend.

Required work:

- extract terminal tab/session lifecycle from `App`
- define shared events for:
  - session connected/disconnected
  - terminal output
  - title updates
  - SFTP listing/preview updates
  - port forwarding status updates

## Phase 4: Management Shell

Goal: port the management surface after the terminal rendering path is established.

Current progress:

- `Cargo.toml` defines the `TimonSlintShell` binary for management-shell migration work
- `src/slint_shell_app.rs` now owns the reusable Slint shell entry point, so both `TimonSlintShell` and the main `Timon` binary can launch the same Slint shell when built with `--features slint-ui`
- `src/slint_shell_app.rs` loads `WorkspaceData` from the existing SQLite-backed persistence layer
- `src/slint_shell_app.rs` renders a first Slint-native management shell with top bar, sidebar, workspace stats, and connection rows
- `src/slint_shell_app.rs` maps `Connection` models into Slint view models for SSH, local shell, and serial targets
- `src/slint_shell_app.rs` renders existing connection groups as read-only cards on the Slint Connections page, including parent group and connection counts
- `src/slint_shell_app.rs` includes connection group names in read-only connection summaries and Connections search filtering
- `src/slint_shell_app.rs` now supports selecting connection rows and showing selected connection details in the Slint shell
- `src/slint_shell_app.rs` now handles sidebar navigation state and renders Slint-native read-only list panels with explicit empty states for migrated management pages
- `src/slint_shell_app.rs` can launch the Slint terminal binary for the selected connection, and `src/slint_terminal_app.rs` accepts `--connection-id` to open a persisted connection with its effective key/identity
- `src/slint_terminal_app.rs` now resolves terminal colors from the selected connection theme id, including settings default colors, built-in themes, custom themes, and atom-one-light fallback
- `src/slint_shell_app.rs` now opens terminals from connection rows directly and guards the Connect action so non-connection pages cannot trigger terminal launches
- `src/slint_shell_app.rs` now launches terminal windows through the current main `Timon` binary when running under the Slint main entry point, and falls back to the sibling `TimonSlintTerminal` wrapper for standalone shell builds
- `src/slint_shell_app.rs` now has a real search input that filters Connections, Keychain, Port Forwarding, and Known Hosts data through Rust-side view models
- `src/slint_shell_app.rs` now renders a real Settings summary page from persisted app settings, including terminal font, theme, scrollback, cursor, and shortcut data
- `src/slint_shell_app.rs` now renders an in-memory Logs page for Slint shell startup, navigation, search, and terminal-launch events, capped like the legacy iced log buffer
- `src/models.rs`, `src/persistence.rs`, `src/workspace.rs`, and `src/slint_shell_app.rs` now include persisted Snippets and render/search them in the Slint shell instead of showing a placeholder page
- `src/main.rs` now launches `slint_shell_app::run()` for the default main `Timon` binary because `slint-ui` is the default feature; the legacy iced application remains available only when building with `--no-default-features --features iced-ui`
- Slint shell is intentionally read-oriented for now: it navigates, searches, displays persisted data, and opens the Slint terminal without adding new create/delete/start/stop behavior during the migration

Suggested scope for the first management Slint shell:

- top bar
- sidebar
- connections list
- drawers for create/edit flows
- logs/settings entry points

## Recommended Order

1. Continue extracting shared core/services from `App`
2. Bring the Slint terminal core to behavior parity with the old iced terminal
3. Expand Slint keyboard handling to match the full old terminal input protocol
4. Port the management UI once terminal behavior has parity

## Practical Constraint

This repository is around 12k lines of Rust, and the terminal subsystem alone is over 3k lines. A direct rewrite to Slint without an intermediate core split would be slower and much riskier than a staged migration.
