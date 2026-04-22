use super::*;

#[cfg(target_os = "macos")]
use iced::window::raw_window_handle::RawWindowHandle;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSGlassEffectView, NSGlassEffectViewStyle, NSView,
    NSWindowOrderingMode,
};
#[cfg(target_os = "macos")]
use objc2_foundation::MainThreadMarker;

pub(crate) fn normalize_selection(anchor: TerminalPoint, head: TerminalPoint) -> TerminalSelection {
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

pub(crate) fn terminal_composer_id(id: u64) -> iced::widget::Id {
    format!("terminal-composer-{id}").into()
}

pub(crate) fn animate_scalar(current: f32, target: f32) -> f32 {
    let next = current + (target - current) * TITLEBAR_TAB_ANIMATION_LERP;
    if (next - target).abs() < TITLEBAR_TAB_ANIMATION_EPSILON {
        target
    } else {
        next
    }
}

pub(crate) fn selection_contents(
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

pub(crate) fn cell_in_selection(
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

pub(crate) fn main_window_settings() -> window::Settings {
    #[allow(unused_mut)]
    let mut settings = window::Settings {
        size: iced::Size::new(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT),
        min_size: Some(iced::Size::new(
            MAIN_WINDOW_MIN_WIDTH,
            MAIN_WINDOW_MIN_HEIGHT,
        )),
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

pub(crate) fn settings_window_settings() -> window::Settings {
    window::Settings {
        size: iced::Size::new(SETTINGS_WINDOW_WIDTH, SETTINGS_WINDOW_HEIGHT),
        min_size: Some(iced::Size::new(
            SETTINGS_WINDOW_MIN_WIDTH,
            SETTINGS_WINDOW_MIN_HEIGHT,
        )),
        ..window::Settings::default()
    }
}

pub(crate) fn install_main_window_vibrancy<Message>(id: window::Id) -> Task<Message>
where
    Message: Send + 'static,
{
    #[cfg(target_os = "macos")]
    {
        return window::run(id, |window| {
            install_macos_vibrancy(window);
        })
        .discard();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = id;
        Task::none()
    }
}

#[cfg(target_os = "macos")]
fn install_macos_vibrancy(window: &dyn window::raw_window_handle::HasWindowHandle) {
    let Ok(window_handle) = window.window_handle() else {
        return;
    };

    let RawWindowHandle::AppKit(handle) = window_handle.as_raw() else {
        return;
    };

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let ns_view: Retained<NSView> =
        unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) }.expect("valid NSView");

    let container = unsafe { ns_view.superview() }.unwrap_or_else(|| ns_view.clone());
    let container_view: &NSView = container.as_ref();
    let target_view: &NSView = ns_view.as_ref();
    let is_same_view = std::ptr::eq::<NSView>(container_view, target_view);

    let frame = if is_same_view {
        ns_view.bounds()
    } else {
        ns_view.frame()
    };

    let effect_view = NSGlassEffectView::new(mtm);

    effect_view.setFrame(frame);
    effect_view.setStyle(NSGlassEffectViewStyle::Regular);
    effect_view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );

    if is_same_view {
        container.addSubview_positioned_relativeTo(
            effect_view.as_ref(),
            NSWindowOrderingMode::Below,
            None,
        );
    } else {
        container.addSubview_positioned_relativeTo(
            effect_view.as_ref(),
            NSWindowOrderingMode::Below,
            Some(ns_view.as_ref()),
        );
    }
}

pub(crate) fn parse_optional_i64(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<i64>().ok()
    }
}

pub(crate) fn available_local_shells() -> Vec<String> {
    #[cfg(windows)]
    {
        let mut shells = vec![
            "pwsh.exe".to_string(),
            "powershell.exe".to_string(),
            "cmd.exe".to_string(),
        ];
        if let Ok(comspec) = std::env::var("COMSPEC") {
            if !shells
                .iter()
                .any(|entry| entry.eq_ignore_ascii_case(&comspec))
            {
                shells.insert(0, comspec);
            }
        }
        return shells;
    }

    #[cfg(not(windows))]
    {
        let mut shells = std::fs::read_to_string("/etc/shells")
            .ok()
            .map(|content| {
                content
                    .lines()
                    .map(str::trim)
                    .filter(|line| {
                        !line.is_empty() && !line.starts_with('#') && line.starts_with('/')
                    })
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if let Ok(shell) = std::env::var("SHELL") {
            if !shells.iter().any(|entry| entry == &shell) {
                shells.insert(0, shell);
            }
        }

        if shells.is_empty() {
            shells = vec![
                "/bin/zsh".into(),
                "/bin/bash".into(),
                "/bin/sh".into(),
                "/opt/homebrew/bin/fish".into(),
            ];
        }

        shells
    }
}

pub(crate) fn default_local_shell_path() -> String {
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "pwsh.exe".into())
    }

    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into())
    }
}

pub(crate) fn shell_display_name(shell_path: &str) -> String {
    let trimmed = shell_path.trim();
    if trimmed.is_empty() {
        "Login Shell".into()
    } else {
        std::path::Path::new(trimmed)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(trimmed)
            .to_string()
    }
}

pub(crate) fn local_terminal_tab_title(connection: &Connection) -> String {
    let user = current_local_username();
    let host = local_machine_name();
    let path = local_work_dir_label(&connection.work_dir);

    format!("{user}@{host}:{path}")
}

fn current_local_username() -> String {
    std::env::var("USER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("USERNAME")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "user".into())
}

fn local_machine_name() -> String {
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        let trimmed = hostname.trim();
        if !trimmed.is_empty() {
            return trimmed.split('.').next().unwrap_or(trimmed).to_string();
        }
    }

    if let Ok(hostname) = std::env::var("COMPUTERNAME") {
        let trimmed = hostname.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| value.split('.').next().unwrap_or(&value).to_string())
        .unwrap_or_else(|| "localhost".into())
}

fn local_work_dir_label(work_dir: &str) -> String {
    let trimmed = work_dir.trim();

    if trimmed.is_empty() {
        return "~".into();
    }

    if let Some(home) = current_home_dir() {
        if trimmed == home {
            return "~".into();
        }

        if let Some(suffix) = trimmed.strip_prefix(&home) {
            if suffix.is_empty() {
                return "~".into();
            }

            return format!("~{suffix}");
        }
    }

    trimmed.to_string()
}

fn current_home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("USERPROFILE")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            let drive = std::env::var("HOMEDRIVE").ok()?;
            let path = std::env::var("HOMEPATH").ok()?;
            let drive = drive.trim();
            let path = path.trim();
            if drive.is_empty() || path.is_empty() {
                None
            } else {
                Some(format!("{drive}{path}"))
            }
        })
}

pub(crate) fn empty_as_dash(value: &str) -> String {
    if value.trim().is_empty() {
        "-".into()
    } else {
        value.to_string()
    }
}

pub(crate) fn short_preview(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "-".into()
    } else if trimmed.len() > 64 {
        format!("{}...", &trimmed[..64])
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn normalize_font_family_choice(current: &str, available_fonts: &[String]) -> String {
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
