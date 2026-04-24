use super::*;
use crate::terminal::TerminalTheme;

pub(crate) fn color_canvas() -> Color {
    Color::from_rgb8(246, 247, 249)
}

pub(crate) fn color_surface_subtle() -> Color {
    Color::from_rgb8(250, 250, 250)
}

pub(crate) fn color_surface_elevated() -> Color {
    Color::from_rgb8(255, 255, 255)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn color_glass_fill() -> Color {
    Color::from_rgba8(230, 236, 242, 0.94)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn color_glass_edge() -> Color {
    Color::from_rgba8(244, 247, 251, 0.98)
}

pub(crate) fn color_text_primary() -> Color {
    Color::from_rgb8(18, 24, 38)
}

pub(crate) fn color_text_secondary() -> Color {
    Color::from_rgb8(107, 114, 128)
}

pub(crate) fn color_text_muted() -> Color {
    Color::from_rgb8(148, 163, 184)
}

pub(crate) fn color_text_faint() -> Color {
    Color::from_rgb8(156, 163, 175)
}

pub(crate) fn manage_glass_text_primary() -> Color {
    Color::from_rgb8(240, 244, 252)
}

pub(crate) fn manage_glass_text_secondary() -> Color {
    Color::from_rgb8(159, 172, 196)
}

pub(crate) fn color_ring_subtle() -> Color {
    Color::from_rgba8(15, 23, 42, 0.06)
}

pub(crate) fn color_ring_soft() -> Color {
    Color::from_rgb8(229, 231, 235)
}

pub(crate) fn color_focus() -> Color {
    Color::from_rgb8(10, 114, 239)
}

pub(crate) fn shallow_shadow() -> Shadow {
    Shadow {
        color: Color::from_rgba8(15, 23, 42, 0.025),
        offset: Vector::new(0.0, 1.0),
        blur_radius: 8.0,
    }
}

pub(crate) fn bordered_surface(
    background: Color,
    radius: f32,
    border_color: Color,
    shadow: Shadow,
) -> container::Style {
    container::Style {
        text_color: Some(color_text_primary()),
        background: Some(Background::Color(background)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: radius.into(),
        },
        shadow,
        ..Default::default()
    }
}

pub(crate) fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let clamped = t.clamp(0.0, 1.0);

    Color {
        r: a.r + (b.r - a.r) * clamped,
        g: a.g + (b.g - a.g) * clamped,
        b: a.b + (b.b - a.b) * clamped,
        a: a.a + (b.a - a.a) * clamped,
    }
}

pub(crate) fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

fn relative_luminance(color: Color) -> f32 {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b
}

fn terminal_chrome_foreground(theme: &TerminalTheme) -> Color {
    theme.foreground
}

fn terminal_chrome_secondary(theme: &TerminalTheme) -> Color {
    mix_color(theme.foreground, theme.background, 0.38)
}

fn terminal_chrome_muted(theme: &TerminalTheme) -> Color {
    mix_color(theme.foreground, theme.background, 0.56)
}

fn terminal_chrome_overlay(theme: &TerminalTheme, alpha: f32) -> Color {
    if relative_luminance(theme.background) < 0.5 {
        with_alpha(Color::WHITE, alpha)
    } else {
        with_alpha(Color::BLACK, alpha)
    }
}

pub(crate) fn app_shell_style() -> container::Style {
    bordered_surface(color_canvas(), 14.0, color_ring_subtle(), shallow_shadow())
}

pub(crate) fn topbar_style() -> container::Style {
    bordered_surface(color_canvas(), 14.0, color_ring_subtle(), Shadow::default())
}

pub(crate) fn terminal_topbar_style(theme: &TerminalTheme) -> container::Style {
    container::Style {
        text_color: Some(theme.foreground),
        background: Some(Background::Color(theme.background)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub(crate) fn manage_shell_style() -> container::Style {
    bordered_surface(
        Color::from_rgb8(236, 242, 244),
        14.0,
        Color::from_rgba8(15, 23, 42, 0.07),
        Shadow {
            color: Color::from_rgba8(15, 23, 42, 0.045),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 24.0,
        },
    )
}

pub(crate) fn manage_topbar_style() -> container::Style {
    container::Style {
        text_color: Some(manage_glass_text_primary()),
        background: Some(Background::Color(Color::from_rgb8(63, 68, 89))),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub(crate) fn manage_sidebar_style() -> container::Style {
    container::Style {
        text_color: Some(color_text_primary()),
        background: Some(Background::Color(Color::from_rgb8(250, 250, 250))),
        border: Border {
            color: Color::from_rgb8(229, 231, 235),
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub(crate) fn manage_content_surface_style() -> container::Style {
    container::Style {
        text_color: Some(color_text_primary()),
        background: Some(Background::Color(Color::from_rgb8(255, 255, 255))),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub(crate) fn drawer_surface_style() -> container::Style {
    bordered_surface(color_canvas(), 0.0, color_ring_subtle(), shallow_shadow())
}

pub(crate) fn card_surface_style() -> container::Style {
    bordered_surface(
        color_surface_elevated(),
        14.0,
        color_ring_soft(),
        shallow_shadow(),
    )
}

pub(crate) fn section_surface_style() -> container::Style {
    bordered_surface(
        color_surface_elevated(),
        14.0,
        color_ring_soft(),
        shallow_shadow(),
    )
}

pub(crate) fn button_style_base(
    background: Color,
    text_color: Color,
    border_color: Color,
    shadow: Shadow,
) -> button::Style {
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow,
        ..Default::default()
    }
}

pub(crate) fn light_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered => button_style_base(
            Color::from_rgb8(248, 250, 252),
            color_text_primary(),
            color_ring_soft(),
            Shadow::default(),
        ),
        button::Status::Pressed => button_style_base(
            Color::from_rgb8(241, 245, 249),
            color_text_primary(),
            color_ring_soft(),
            Shadow::default(),
        ),
        button::Status::Disabled => button_style_base(
            color_surface_subtle(),
            color_text_faint(),
            color_ring_soft(),
            Shadow::default(),
        ),
        _ => button_style_base(
            Color::from_rgb8(255, 255, 255),
            color_text_primary(),
            color_ring_soft(),
            Shadow::default(),
        ),
    }
}

pub(crate) fn dark_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered => button_style_base(
            Color::from_rgb8(34, 86, 244),
            color_canvas(),
            Color::from_rgb8(34, 86, 244),
            Shadow::default(),
        ),
        button::Status::Pressed => button_style_base(
            Color::from_rgb8(26, 74, 222),
            color_canvas(),
            Color::from_rgb8(26, 74, 222),
            Shadow::default(),
        ),
        button::Status::Disabled => button_style_base(
            Color::from_rgb8(210, 210, 210),
            color_canvas(),
            Color::from_rgb8(210, 210, 210),
            Shadow::default(),
        ),
        _ => button_style_base(
            Color::from_rgb8(37, 99, 235),
            color_canvas(),
            Color::from_rgb8(37, 99, 235),
            Shadow::default(),
        ),
    }
}

pub(crate) fn floating_menu_button_style(
    active: bool,
    _theme: &Theme,
    status: button::Status,
) -> button::Style {
    let background = match status {
        button::Status::Hovered if active => Color::from_rgb8(255, 255, 255),
        button::Status::Pressed if active => Color::from_rgb8(248, 250, 252),
        button::Status::Hovered => Color::from_rgba8(255, 255, 255, 0.08),
        button::Status::Pressed => Color::from_rgba8(255, 255, 255, 0.12),
        _ if active => Color::from_rgb8(255, 255, 255),
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active {
            color_focus()
        } else {
            manage_glass_text_secondary()
        },
        border: Border {
            color: if active {
                Color::from_rgb8(226, 232, 240)
            } else {
                Color::TRANSPARENT
            },
            width: if active { 1.0 } else { 0.0 },
            radius: 12.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub(crate) fn terminal_titlebar_tab_container_style(_active: bool) -> container::Style {
    container::Style {
        text_color: None,
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub(crate) fn titlebar_tab_container_style(active: bool, floating: bool) -> container::Style {
    if floating {
        return container::Style {
            text_color: Some(color_text_primary()),
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        };
    }

    bordered_surface(
        if active {
            color_canvas()
        } else {
            Color::from_rgba8(23, 23, 23, 0.08)
        },
        10.0,
        Color::from_rgba8(23, 23, 23, 0.14),
        if active {
            shallow_shadow()
        } else {
            Shadow::default()
        },
    )
}

pub(crate) fn terminal_titlebar_mode_badge_style(
    active: bool,
    theme: &TerminalTheme,
) -> container::Style {
    bordered_surface(
        terminal_chrome_overlay(theme, if active { 0.16 } else { 0.1 }),
        5.0,
        terminal_chrome_overlay(theme, if active { 0.2 } else { 0.14 }),
        Shadow::default(),
    )
}

pub(crate) fn titlebar_mode_badge_style(active: bool, floating: bool) -> container::Style {
    if floating {
        return bordered_surface(
            if active {
                Color::from_rgba8(255, 255, 255, 0.12)
            } else {
                Color::from_rgba8(255, 255, 255, 0.08)
            },
            999.0,
            if active {
                Color::from_rgba8(255, 255, 255, 0.18)
            } else {
                Color::from_rgba8(255, 255, 255, 0.12)
            },
            Shadow::default(),
        );
    }

    bordered_surface(
        if active {
            Color::from_rgba8(23, 23, 23, 0.10)
        } else {
            Color::from_rgba8(23, 23, 23, 0.06)
        },
        5.0,
        Color::from_rgba8(23, 23, 23, 0.12),
        Shadow::default(),
    )
}

pub(crate) fn terminal_titlebar_tab_button_style(
    active: bool,
    theme: &TerminalTheme,
    _theme_ctx: &Theme,
    status: button::Status,
) -> button::Style {
    let background = match status {
        button::Status::Hovered => terminal_chrome_overlay(theme, if active { 0.14 } else { 0.1 }),
        button::Status::Pressed => terminal_chrome_overlay(theme, if active { 0.18 } else { 0.14 }),
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active {
            terminal_chrome_foreground(theme)
        } else {
            terminal_chrome_secondary(theme)
        },
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 10.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

pub(crate) fn titlebar_tab_button_style(
    active: bool,
    floating: bool,
    _theme: &Theme,
    status: button::Status,
) -> button::Style {
    if floating {
        let background = match status {
            button::Status::Hovered => {
                Color::from_rgba8(255, 255, 255, if active { 0.16 } else { 0.10 })
            }
            button::Status::Pressed => {
                Color::from_rgba8(255, 255, 255, if active { 0.20 } else { 0.13 })
            }
            _ if active => Color::from_rgba8(255, 255, 255, 0.13),
            _ => Color::from_rgba8(255, 255, 255, 0.07),
        };

        return button::Style {
            background: Some(Background::Color(background)),
            text_color: if active {
                Color::from_rgb8(245, 248, 255)
            } else {
                Color::from_rgb8(176, 188, 209)
            },
            border: Border {
                color: if active {
                    Color::from_rgba8(255, 255, 255, 0.10)
                } else {
                    Color::from_rgba8(255, 255, 255, 0.06)
                },
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        };
    }

    let background = match status {
        button::Status::Hovered if !active => Color::from_rgba8(23, 23, 23, 0.03),
        button::Status::Pressed if !active => Color::from_rgba8(23, 23, 23, 0.05),
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active {
            color_text_primary()
        } else {
            color_text_secondary()
        },
        border: Border {
            color: Color::TRANSPARENT,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

pub(crate) fn terminal_titlebar_close_button_style(
    theme: &TerminalTheme,
    _theme_ctx: &Theme,
    status: button::Status,
) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    let pressed = matches!(status, button::Status::Pressed);

    button::Style {
        background: Some(Background::Color(if pressed {
            terminal_chrome_overlay(theme, 0.18)
        } else if hovered {
            terminal_chrome_overlay(theme, 0.12)
        } else {
            Color::TRANSPARENT
        })),
        text_color: terminal_chrome_muted(theme),
        border: Border {
            color: terminal_chrome_overlay(theme, if hovered { 0.24 } else { 0.14 }),
            width: 1.0,
            radius: 999.0.into(),
        },
        ..Default::default()
    }
}

pub(crate) fn titlebar_close_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered);
    let pressed = matches!(status, button::Status::Pressed);

    button::Style {
        background: Some(Background::Color(if pressed {
            Color::from_rgba8(23, 23, 23, 0.12)
        } else if hovered {
            Color::from_rgba8(23, 23, 23, 0.08)
        } else {
            Color::TRANSPARENT
        })),
        text_color: color_text_muted(),
        border: Border {
            color: Color::from_rgba8(23, 23, 23, if hovered { 0.22 } else { 0.14 }),
            width: 1.0,
            radius: 999.0.into(),
        },
        ..Default::default()
    }
}

pub(crate) fn terminal_chrome_text_color(theme: &TerminalTheme, active: bool) -> Color {
    if active {
        terminal_chrome_foreground(theme)
    } else {
        terminal_chrome_secondary(theme)
    }
}

pub(crate) fn terminal_chrome_muted_color(theme: &TerminalTheme) -> Color {
    terminal_chrome_muted(theme)
}

pub(crate) fn field_style(
    _theme: &Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let border_color = match status {
        iced::widget::text_input::Status::Focused { .. } => color_focus(),
        iced::widget::text_input::Status::Hovered => color_text_muted(),
        _ => color_ring_soft(),
    };

    iced::widget::text_input::Style {
        background: Background::Color(color_canvas()),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 8.0.into(),
        },
        icon: color_text_muted(),
        placeholder: color_text_faint(),
        value: color_text_primary(),
        selection: Color::from_rgb8(235, 245, 255),
    }
}

pub(crate) fn pick_list_field_style(
    _theme: &Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let border_color = match status {
        iced::widget::pick_list::Status::Opened { .. }
        | iced::widget::pick_list::Status::Hovered => color_focus(),
        _ => color_ring_soft(),
    };

    iced::widget::pick_list::Style {
        text_color: color_text_primary(),
        placeholder_color: color_text_faint(),
        handle_color: color_text_muted(),
        background: Background::Color(color_canvas()),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 8.0.into(),
        },
    }
}

pub(crate) fn pick_list_menu_style(_theme: &Theme) -> iced::widget::overlay::menu::Style {
    iced::widget::overlay::menu::Style {
        background: Background::Color(Color::from_rgb8(255, 255, 255)),
        border: Border {
            color: color_ring_soft(),
            width: 1.0,
            radius: 10.0.into(),
        },
        text_color: color_text_primary(),
        selected_text_color: color_focus(),
        selected_background: Background::Color(Color::from_rgb8(241, 245, 249)),
        shadow: Shadow {
            color: Color::from_rgba8(15, 23, 42, 0.07),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 28.0,
        },
    }
}

pub(crate) fn text_editor_field_style(
    _theme: &Theme,
    status: iced::widget::text_editor::Status,
) -> iced::widget::text_editor::Style {
    let border_color = match status {
        iced::widget::text_editor::Status::Focused { .. } => color_focus(),
        iced::widget::text_editor::Status::Hovered => color_text_muted(),
        _ => color_ring_soft(),
    };

    iced::widget::text_editor::Style {
        background: Background::Color(color_canvas()),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 8.0.into(),
        },
        placeholder: color_text_faint(),
        value: color_text_primary(),
        selection: Color::from_rgb8(235, 245, 255),
    }
}

pub(crate) fn terminal_composer_area_style(theme: &TerminalTheme) -> container::Style {
    container::Style {
        text_color: Some(theme.foreground),
        background: Some(Background::Color(theme.background)),
        ..Default::default()
    }
}

pub(crate) fn terminal_composer_style(
    terminal_theme: &TerminalTheme,
    _theme: &Theme,
    status: iced::widget::text_editor::Status,
) -> iced::widget::text_editor::Style {
    let border_color = match status {
        iced::widget::text_editor::Status::Focused { .. } => terminal_theme.cursor_color,
        iced::widget::text_editor::Status::Hovered => {
            mix_color(terminal_theme.foreground, terminal_theme.background, 0.42)
        }
        _ => mix_color(terminal_theme.foreground, terminal_theme.background, 0.72),
    };

    iced::widget::text_editor::Style {
        background: Background::Color(mix_color(
            terminal_theme.background,
            terminal_theme.foreground,
            0.05,
        )),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 10.0.into(),
        },
        placeholder: mix_color(terminal_theme.foreground, terminal_theme.background, 0.52),
        value: terminal_theme.foreground,
        selection: with_alpha(terminal_theme.cursor_color, 0.18),
    }
}
