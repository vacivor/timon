use super::*;
use iced::widget::column;

pub(crate) fn title(app: &App, window: window::Id) -> String {
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

pub(crate) fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::batch([
        time::every(Duration::from_millis(TICK_MS)).map(|_| Message::Tick),
        window::events().map(|(id, event)| Message::WindowEvent(id, event)),
        event::listen_with(|event, status, window| match (status, event) {
            (event::Status::Ignored, iced::Event::Keyboard(key_event)) => {
                Some(Message::KeyboardInput(window, key_event))
            }
            (event::Status::Ignored, iced::Event::InputMethod(ime_event)) => {
                Some(Message::InputMethod(window, ime_event))
            }
            _ => None,
        }),
    ])
}

pub(crate) fn theme(_app: &App, _window: window::Id) -> Theme {
    Theme::Light
}

pub(crate) fn style(_app: &App, _theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: color_text_primary(),
    }
}

pub(crate) fn view(app: &App, window: window::Id) -> Element<'_, Message> {
    if Some(window) == app.settings_window {
        return settings_window_view(app).into();
    }

    main_window_view(app, window).into()
}

fn main_window_view<'a>(app: &'a App, window: window::Id) -> iced::widget::Container<'a, Message> {
    let manage_active = app.active_tab == ActiveTab::Manage;
    let active_terminal_theme = match app.active_tab {
        ActiveTab::Terminal(id) => app
            .terminal_tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| app.terminal_theme(&tab.theme_id)),
        ActiveTab::Manage => None,
    };
    let topbar_terminal_theme = active_terminal_theme.clone();
    let strip_terminal_theme = active_terminal_theme.clone();
    let controls_terminal_theme = active_terminal_theme.clone();
    let top_bar = container(
        row![
            mac_titlebar_spacer(),
            titlebar_tab_strip(app, strip_terminal_theme),
            mouse_area(
                container(
                    Space::new()
                        .width(Length::Fill)
                        .height(Length::Fixed(TITLEBAR_HEIGHT))
                )
                .width(Length::Fill)
            )
            .on_press(Message::DragWindow(window)),
            titlebar_controls(app, controls_terminal_theme),
        ]
        .align_y(Vertical::Center)
        .spacing(10),
    )
    .padding([TITLEBAR_PADDING_Y, TITLEBAR_PADDING_X])
    .height(Length::Fixed(TITLEBAR_HEIGHT))
    .style(move |_| {
        if manage_active {
            manage_topbar_style()
        } else if let Some(theme) = topbar_terminal_theme.as_ref() {
            terminal_topbar_style(theme)
        } else {
            topbar_style()
        }
    });

    let body: Element<'_, _> = match app.active_tab {
        ActiveTab::Manage => manage_page_view(app).into(),
        ActiveTab::Terminal(id) => terminal_page_view(app, id),
    };

    let shell = column![top_bar, body]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    container(shell)
        .padding(1)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| {
            if manage_active {
                manage_shell_style()
            } else {
                app_shell_style()
            }
        })
}

fn titlebar_tab_strip<'a>(
    app: &'a App,
    terminal_theme: Option<TerminalTheme>,
) -> Element<'a, Message> {
    let floating = app.active_tab == ActiveTab::Manage;
    let manage_button_theme = terminal_theme.clone();
    let manage = container(
        button(
            row![
                text("Manage")
                    .size(13)
                    .color(if let Some(theme) = terminal_theme.as_ref() {
                        terminal_chrome_text_color(theme, app.active_tab == ActiveTab::Manage)
                    } else if app.active_tab == ActiveTab::Manage {
                        manage_glass_text_primary()
                    } else {
                        manage_glass_text_secondary()
                    }),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .style(move |theme_ctx, status| {
            if let Some(terminal_theme) = manage_button_theme.as_ref() {
                terminal_titlebar_tab_button_style(
                    app.active_tab == ActiveTab::Manage,
                    terminal_theme,
                    theme_ctx,
                    status,
                )
            } else {
                titlebar_tab_button_style(
                    app.active_tab == ActiveTab::Manage,
                    floating,
                    theme_ctx,
                    status,
                )
            }
        })
        .on_press(Message::ActivateManageTab),
    )
    .width(Length::Fixed(app.manage_tab_width));

    let tabs = terminal_tab_buttons(app, terminal_theme.clone());
    let separator: Element<'_, Message> = if app.terminal_tabs.is_empty() {
        Space::new().width(Length::Shrink).into()
    } else {
        container(
            Space::new()
                .width(Length::Fixed(1.0))
                .height(Length::Fixed(18.0)),
        )
        .style(|_| {
            bordered_surface(
                Color::from_rgba8(255, 255, 255, 0.16),
                1.0,
                Color::TRANSPARENT,
                Shadow::default(),
            )
        })
        .into()
    };

    container(
        row![manage, separator, tabs]
            .spacing(8)
            .align_y(Vertical::Center),
    )
    .width(Length::FillPortion(3))
    .into()
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
            labeled_input("Scrollback Lines", &settings.scrollback_lines, |value| {
                Message::SettingsScrollbackChanged(value)
            }),
            button(if settings.font_thicken {
                "Font Thicken: On"
            } else {
                "Font Thicken: Off"
            })
            .style(light_button_style)
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
            .style(light_button_style)
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
            labeled_input("Cursor Color", &settings.cursor_color, |value| {
                Message::SettingsColorChanged(ColorField::CursorColor, value)
            }),
            labeled_input("Cursor Text", &settings.cursor_text, |value| {
                Message::SettingsColorChanged(ColorField::CursorText, value)
            }),
            labeled_input(
                "Selection Background",
                &settings.selection_background,
                |value| { Message::SettingsColorChanged(ColorField::SelectionBackground, value) }
            ),
            labeled_input(
                "Selection Foreground",
                &settings.selection_foreground,
                |value| { Message::SettingsColorChanged(ColorField::SelectionForeground, value) }
            ),
            ansi_color_inputs(&settings.ansi_normal, &settings.ansi_bright),
        ]
        .spacing(10),
    );

    container(
        column![
            row![
                column![
                    text("Settings").size(30).color(color_text_primary()),
                    text("Terminal theme, cursor, font, and ANSI palette.")
                        .size(14)
                        .color(color_text_secondary()),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                button("Reset to Atom One Light")
                    .style(light_button_style)
                    .on_press(Message::ResetThemeToAtomOneLight),
                button("Save")
                    .style(dark_button_style)
                    .on_press(Message::SaveSettings),
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
        .padding(24),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| {
        bordered_surface(
            color_surface_subtle(),
            0.0,
            color_ring_subtle(),
            Shadow::default(),
        )
    })
}

fn manage_page_view(app: &App) -> iced::widget::Container<'_, Message> {
    let sidebar = container(column![menu_buttons(app),].spacing(0).padding(0))
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Length::Fill)
        .style(|_| manage_sidebar_style());

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
        .padding(24)
        .style(|_| manage_content_surface_style());

    let base: Element<'_, Message> = row![sidebar, content]
        .spacing(18)
        .height(Length::Fill)
        .width(Length::Fill)
        .into();

    let layered: Element<'_, Message> = if let Some(drawer) = &app.drawer {
        stack([
            base,
            opaque(
                row![
                    mouse_area(
                        container(Space::new().width(Length::Fill).height(Length::Fill))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .style(|_| {
                                bordered_surface(
                                    Color::from_rgba8(15, 23, 42, 0.08),
                                    0.0,
                                    Color::TRANSPARENT,
                                    Shadow::default(),
                                )
                            })
                    )
                    .on_press(Message::CloseDrawer),
                    drawer_view(app, drawer),
                ]
                .height(Length::Fill)
                .width(Length::Fill),
            ),
        ])
        .into()
    } else {
        base
    };

    container(layered).width(Length::Fill).height(Length::Fill)
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
        app.active_tab == ActiveTab::Terminal(id)
            && app.terminal_focus == Some(id)
            && app.terminal_composer_focus != Some(id),
        Arc::new(move |event| match event {
            TerminalCanvasEvent::SelectionStarted(point) => {
                Message::TerminalSelectionStarted(id, point)
            }
            TerminalCanvasEvent::SelectionUpdated(point) => {
                Message::TerminalSelectionUpdated(id, point)
            }
            TerminalCanvasEvent::Scrolled { lines, point } => {
                Message::TerminalScrolled(id, lines, point)
            }
        }),
    )
    .element();

    let composer_height = app
        .terminal_composer_editor_height(app.terminal_composer_visual_lines(&tab.composer.text()));
    let composer_content: Element<'_, Message> = text_editor(&tab.composer)
        .id(terminal_composer_id(id))
        .placeholder("Type a command...")
        .on_action(move |action| Message::TerminalComposerAction(id, action))
        .font(app.terminal_font().iced_font())
        .key_binding(move |keypress| {
            if !matches!(
                keypress.status,
                iced::widget::text_editor::Status::Focused { .. }
            ) {
                return None;
            }

            if matches!(
                keypress.key,
                keyboard::Key::Named(keyboard::key::Named::Enter)
            ) {
                if keypress.modifiers.shift() {
                    Some(iced::widget::text_editor::Binding::Enter)
                } else {
                    Some(iced::widget::text_editor::Binding::Custom(
                        Message::SubmitTerminalComposer(id),
                    ))
                }
            } else {
                iced::widget::text_editor::Binding::from_key_press(keypress)
            }
        })
        .height(Length::Fixed(composer_height))
        .size(TERMINAL_COMPOSER_FONT_SIZE)
        .line_height(iced::Pixels(TERMINAL_COMPOSER_LINE_HEIGHT))
        .padding([
            TERMINAL_COMPOSER_INNER_PADDING_Y,
            TERMINAL_COMPOSER_INNER_PADDING_X,
        ])
        .style(terminal_composer_style)
        .into();

    let composer = container(container(composer_content).width(Length::Fill)).padding([
        TERMINAL_COMPOSER_PADDING_Y,
        TERMINAL_COMPOSER_HORIZONTAL_PADDING,
    ]);

    container(column![
        container(terminal).width(Length::Fill).height(Length::Fill),
        composer
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn profiles_view(app: &App) -> Element<'_, Message> {
    const PROFILE_GRID_COLUMN_GAP: f32 = 14.0;
    const PROFILE_GRID_ROW_GAP: f32 = 10.0;

    let header = row![
        column![
            text("Profiles").size(32).color(color_text_primary()),
            text("Saved SSH targets and local shells.")
                .size(14)
                .color(color_text_secondary()),
        ]
        .spacing(4),
        Space::new().width(Length::Fill),
        button("New Profile")
            .style(dark_button_style)
            .on_press(Message::NewProfile),
    ]
    .align_y(Vertical::Center);

    let cards: Element<'_, Message> = if app.profiles.is_empty() {
        container(
            text("No profiles yet.")
                .size(15)
                .color(color_text_secondary()),
        )
        .padding(20)
        .style(|_| section_surface_style())
        .into()
    } else {
        iced::widget::responsive(move |size| {
            let available_cards_width = size.width.max(PROFILE_CARD_MIN_WIDTH);
            let max_fit_columns = ((available_cards_width + PROFILE_GRID_COLUMN_GAP)
                / (PROFILE_CARD_MIN_WIDTH + PROFILE_GRID_COLUMN_GAP))
                .floor()
                .max(1.0) as usize;
            let profile_columns = max_fit_columns.min(app.profiles.len().max(1));
            let profile_card_width = ((available_cards_width
                - PROFILE_GRID_COLUMN_GAP * (profile_columns.saturating_sub(1) as f32))
                / profile_columns as f32)
                .clamp(PROFILE_CARD_MIN_WIDTH, PROFILE_CARD_MAX_WIDTH);
            let title_max_width = (profile_card_width - 98.0).max(48.0);

            let rows = app.profiles.chunks(profile_columns).fold(
                column![].spacing(PROFILE_GRID_ROW_GAP),
                |column, chunk| {
                    let row = chunk.iter().fold(
                        row![].spacing(PROFILE_GRID_COLUMN_GAP),
                        |row, profile| {
                            row.push(
                                container(profile_card(app, profile, title_max_width))
                                    .width(Length::Fixed(profile_card_width))
                                    .height(Length::Fixed(PROFILE_CARD_HEIGHT)),
                            )
                        },
                    );

                    column.push(row)
                },
            );

            container(rows).width(Length::Fill).into()
        })
        .into()
    };

    scrollable(column![header, cards].spacing(18))
        .height(Length::Fill)
        .into()
}

fn keychain_view(app: &App) -> Element<'_, Message> {
    let keys = app.keys.iter().fold(
        column![
            row![
                column![
                    text("Keys").size(26).color(color_text_primary()),
                    text("Reusable private keys, public keys, and certificates.")
                        .size(14)
                        .color(color_text_secondary()),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                button("New Key")
                    .style(light_button_style)
                    .on_press(Message::NewKey),
            ]
            .align_y(Vertical::Center)
        ]
        .spacing(12),
        |column, key| column.push(key_card(app, key)),
    );

    let identities = app.identities.iter().fold(
        column![
            row![
                column![
                    text("Identities").size(26).color(color_text_primary()),
                    text("Credentials layered on top of keys.")
                        .size(14)
                        .color(color_text_secondary()),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                button("New Identity")
                    .style(light_button_style)
                    .on_press(Message::NewIdentity),
            ]
            .align_y(Vertical::Center)
        ]
        .spacing(12),
        |column, identity| column.push(identity_card(app, identity)),
    );

    scrollable(column![keys, identities].spacing(24))
        .height(Length::Fill)
        .into()
}

fn known_hosts_view(app: &App) -> Element<'_, Message> {
    let list = if app.known_hosts.is_empty() {
        column![
            text("No known hosts yet.")
                .size(15)
                .color(color_text_secondary())
        ]
    } else {
        app.known_hosts
            .iter()
            .fold(column![].spacing(8), |column, entry| {
                column.push(
                    container(
                        column![
                            text(format!("Line {}", entry.line_number))
                                .size(12)
                                .font(iced::Font::MONOSPACE)
                                .color(color_text_muted()),
                            text(entry.line.clone())
                                .font(iced::Font::MONOSPACE)
                                .size(14)
                                .color(color_text_primary()),
                        ]
                        .spacing(4),
                    )
                    .padding(14)
                    .style(|_| card_surface_style()),
                )
            })
    };

    scrollable(
        column![
            text("Known Hosts").size(32).color(color_text_primary()),
            text(app.paths.known_hosts.display().to_string())
                .size(14)
                .font(iced::Font::MONOSPACE)
                .color(color_text_secondary()),
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
                    .padding(12)
                    .style(|_| card_surface_style()),
            )
        });

    scrollable(
        column![
            text("Logs").size(32).color(color_text_primary()),
            text("Runtime activity and persistence events.")
                .size(14)
                .color(color_text_secondary()),
            entries,
        ]
        .spacing(12),
    )
    .height(Length::Fill)
    .into()
}

fn placeholder_view<'a>(title: &'a str, body: &'a str) -> iced::widget::Container<'a, Message> {
    container(
        column![
            text(title).size(32).color(color_text_primary()),
            text(body).size(16).color(color_text_secondary())
        ]
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Left),
    )
    .width(Length::Fill)
    .height(Length::Fill)
}

fn drawer_view<'a>(app: &'a App, drawer: &'a DrawerState) -> iced::widget::Container<'a, Message> {
    let content: Element<'_, _> = match drawer {
        DrawerState::Profile(editor) => profile_drawer(app, editor).into(),
        DrawerState::Key(editor) => key_drawer(editor).into(),
        DrawerState::Identity(editor) => identity_drawer(app, editor).into(),
    };

    container(
        container(content)
            .width(Length::Fixed(DRAWER_WIDTH))
            .height(Length::Fill)
            .style(|_| drawer_surface_style()),
    )
    .padding([12, 12])
    .width(Length::Shrink)
    .height(Length::Fill)
}

fn profile_card<'a>(
    app: &'a App,
    profile: &'a Profile,
    title_max_width: f32,
) -> Element<'a, Message> {
    let type_label = if profile.profile_type == ProfileType::Local {
        "local"
    } else {
        "ssh"
    };
    let connection_label = if profile.profile_type == ProfileType::Local {
        format!("Local PTY · {}", shell_display_name(&profile.shell_path))
    } else {
        format!(
            "{}@{}:{}",
            empty_as_dash(&profile.display_username),
            empty_as_dash(&profile.host),
            profile.port
        )
    };

    let body = column![
        row![
            column![
                text(truncate_to_width(&profile.name, title_max_width, 18.0))
                    .size(18)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .color(color_text_primary()),
                text(connection_label)
                    .size(13)
                    .color(color_text_secondary()),
            ]
            .width(Length::Fill)
            .spacing(3),
            Space::new().width(Length::Fill),
            container(
                text(type_label.to_uppercase())
                    .size(11)
                    .font(iced::Font::MONOSPACE)
                    .color(Color::from_rgb8(0, 104, 214)),
            )
            .padding([4, 10])
            .style(|_| status_badge_style()),
        ],
        row![
            container(
                text(if profile.profile_type == ProfileType::Local {
                    shell_display_name(&profile.shell_path)
                } else {
                    empty_as_dash(&profile.host)
                })
                .size(11)
                .font(iced::Font::MONOSPACE)
                .color(color_text_muted()),
            )
            .padding([4, 10])
            .style(|_| neutral_badge_style()),
            container(
                text(if profile.profile_type == ProfileType::Local {
                    if profile.work_dir.trim().is_empty() {
                        "Home".to_string()
                    } else {
                        profile.work_dir.clone()
                    }
                } else {
                    empty_as_dash(&profile.display_username)
                })
                .size(11)
                .font(iced::Font::MONOSPACE)
                .color(color_text_muted()),
            )
            .padding([4, 10])
            .style(|_| neutral_badge_style()),
        ]
        .spacing(8),
    ]
    .spacing(10);

    let shell: Element<'a, Message> = mouse_area(
        container(body)
            .padding([14, 16])
            .width(Length::Fill)
            .height(Length::Fixed(PROFILE_CARD_HEIGHT))
            .style(|_| capsule_surface_style()),
    )
    .on_right_press(Message::OpenProfileContext(profile.id))
    .into();

    if app.active_profile_context == Some(profile.id) {
        stack([
            shell,
            opaque(
                container(
                    row![
                        Space::new().width(Length::Fill),
                        profile_context_menu(profile.id),
                    ]
                    .align_y(Vertical::Top),
                )
                .padding(10)
                .width(Length::Fill)
                .height(Length::Fill),
            ),
        ])
        .into()
    } else {
        shell
    }
}

fn truncate_to_width(value: &str, max_width: f32, font_size: f32) -> String {
    if estimate_text_width(value, font_size) <= max_width {
        return value.to_string();
    }

    let ellipsis = "...";
    let ellipsis_width = estimate_text_width(ellipsis, font_size);

    if ellipsis_width >= max_width {
        return ellipsis.to_string();
    }

    let mut current = String::new();

    for ch in value.chars() {
        let next_width = estimate_text_width(&format!("{current}{ch}"), font_size);

        if next_width + ellipsis_width > max_width {
            break;
        }

        current.push(ch);
    }

    format!("{current}{ellipsis}")
}

fn estimate_text_width(value: &str, font_size: f32) -> f32 {
    value
        .chars()
        .map(|ch| estimated_char_width(ch) * font_size)
        .sum()
}

fn estimated_char_width(ch: char) -> f32 {
    match ch {
        ' '..='~' if ch.is_ascii_uppercase() => 0.62,
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

fn profile_context_menu(profile_id: i64) -> Element<'static, Message> {
    container(
        column![
            button("Connect")
                .width(Length::Fill)
                .style(light_button_style)
                .on_press(Message::ConnectProfile(profile_id)),
            button("Duplicate")
                .width(Length::Fill)
                .style(light_button_style)
                .on_press(Message::DuplicateProfile(profile_id)),
            button("Edit")
                .width(Length::Fill)
                .style(light_button_style)
                .on_press(Message::EditProfile(profile_id)),
        ]
        .spacing(6),
    )
    .padding(8)
    .width(Length::Fixed(132.0))
    .style(|_| context_menu_style())
    .into()
}

fn key_card<'a>(app: &'a App, key: &'a SshKey) -> Element<'a, Message> {
    let actions: Element<'a, Message> = if app.active_key_context == Some(key.id) {
        container(
            row![
                button("Edit")
                    .style(light_button_style)
                    .on_press(Message::EditKey(key.id))
            ]
            .spacing(8),
        )
        .into()
    } else {
        Space::new().height(Length::Shrink).into()
    };

    let body = column![
        row![
            text(key.name.clone()).size(20).color(color_text_primary()),
            Space::new().width(Length::Fill),
            text(format!("#{}", key.id))
                .size(12)
                .font(iced::Font::MONOSPACE)
                .color(color_text_muted()),
        ],
        text(short_preview(&key.public_key))
            .font(iced::Font::MONOSPACE)
            .size(13)
            .color(color_text_secondary()),
        text(if key.certificate.trim().is_empty() {
            "certificate: -".to_string()
        } else {
            "certificate: attached".to_string()
        })
        .size(12)
        .font(iced::Font::MONOSPACE)
        .color(color_text_muted()),
        actions,
    ]
    .spacing(12);

    mouse_area(container(body).padding(18).style(|_| card_surface_style()))
        .on_right_press(Message::OpenKeyContext(key.id))
        .into()
}

fn identity_card<'a>(app: &'a App, identity: &'a Identity) -> Element<'a, Message> {
    let actions: Element<'a, Message> = if app.active_identity_context == Some(identity.id) {
        container(
            row![
                button("Edit")
                    .style(light_button_style)
                    .on_press(Message::EditIdentity(identity.id))
            ]
            .spacing(8),
        )
        .into()
    } else {
        Space::new().height(Length::Shrink).into()
    };

    let body = column![
        row![
            text(identity.name.clone())
                .size(20)
                .color(color_text_primary()),
            Space::new().width(Length::Fill),
            text(format!("#{}", identity.id))
                .size(12)
                .font(iced::Font::MONOSPACE)
                .color(color_text_muted()),
        ],
        text(format!("username: {}", empty_as_dash(&identity.username)))
            .size(14)
            .color(color_text_secondary()),
        text(format!(
            "key_id: {}",
            identity
                .key_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".into())
        ))
        .size(12)
        .font(iced::Font::MONOSPACE)
        .color(color_text_muted()),
        actions,
    ]
    .spacing(12);

    mouse_area(container(body).padding(18).style(|_| card_surface_style()))
        .on_right_press(Message::OpenIdentityContext(identity.id))
        .into()
}

fn profile_drawer<'a>(
    app: &'a App,
    editor: &'a ProfileEditor,
) -> iced::widget::Container<'a, Message> {
    let type_options = vec!["ssh".to_string(), "local".to_string()];
    let key_options = std::iter::once("None".to_string())
        .chain(app.keys.iter().map(|key| key.id.to_string()))
        .collect::<Vec<_>>();
    let identity_options = std::iter::once("None".to_string())
        .chain(
            app.identities
                .iter()
                .map(|identity| identity.id.to_string()),
        )
        .collect::<Vec<_>>();
    let selected_key = key_options
        .iter()
        .find(|option| option.as_str() == editor.key_id.as_str())
        .cloned()
        .or_else(|| key_options.first().cloned());
    let selected_identity = identity_options
        .iter()
        .find(|option| option.as_str() == editor.identity_id.as_str())
        .cloned()
        .or_else(|| identity_options.first().cloned());
    let mut shell_options = app.available_shells.clone();
    if !editor.shell_path.trim().is_empty()
        && !shell_options
            .iter()
            .any(|option| option.as_str() == editor.shell_path.trim())
    {
        shell_options.push(editor.shell_path.clone());
    }
    shell_options.insert(0, "Login Shell".to_string());
    let selected_type = type_options
        .iter()
        .find(|option| option.as_str() == editor.profile_type.as_str())
        .cloned();
    let selected_shell = if editor.shell_path.trim().is_empty() {
        Some("Login Shell".to_string())
    } else {
        shell_options
            .iter()
            .find(|option| option.as_str() == editor.shell_path.as_str())
            .cloned()
    };
    let ssh_fields: Vec<Element<'a, Message>> = vec![
        labeled_pick_list_owned(
            "Key ID",
            key_options,
            selected_key,
            "Select a key",
            |value| Message::ProfileFieldChanged(ProfileField::KeyId, value),
        )
        .into(),
        labeled_pick_list_owned(
            "Identity ID",
            identity_options,
            selected_identity,
            "Select an identity",
            |value| Message::ProfileFieldChanged(ProfileField::IdentityId, value),
        )
        .into(),
        labeled_input("Host", &editor.host, |value| {
            Message::ProfileFieldChanged(ProfileField::Host, value)
        })
        .into(),
        labeled_input("Port", &editor.port, |value| {
            Message::ProfileFieldChanged(ProfileField::Port, value)
        })
        .into(),
        labeled_input("Username", &editor.username, |value| {
            Message::ProfileFieldChanged(ProfileField::Username, value)
        })
        .into(),
        labeled_input("Password", &editor.password, |value| {
            Message::ProfileFieldChanged(ProfileField::Password, value)
        })
        .into(),
    ];
    let local_fields: Vec<Element<'a, Message>> = vec![
        labeled_pick_list_owned(
            "Shell",
            shell_options,
            selected_shell,
            "Select a shell",
            |value| Message::ProfileFieldChanged(ProfileField::ShellPath, value),
        )
        .into(),
        labeled_input("Work Dir", &editor.work_dir, |value| {
            Message::ProfileFieldChanged(ProfileField::WorkDir, value)
        })
        .into(),
    ];

    let mut content = column![
        labeled_input("Name", &editor.name, |value| {
            Message::ProfileFieldChanged(ProfileField::Name, value)
        }),
        labeled_pick_list_owned(
            "Type",
            type_options,
            selected_type,
            "Select a type",
            |value| { Message::ProfileTypeChanged(value) }
        ),
        labeled_input("Group ID", &editor.group_id, |value| {
            Message::ProfileFieldChanged(ProfileField::GroupId, value)
        }),
    ]
    .spacing(10);

    let section_fields = if editor.profile_type == ProfileType::Local {
        local_fields
    } else {
        ssh_fields
    };

    for field in section_fields {
        content = content.push(field);
    }

    content = content
        .push(labeled_input("Theme ID", &editor.theme_id, |value| {
            Message::ProfileFieldChanged(ProfileField::ThemeId, value)
        }))
        .push(labeled_input(
            "Startup Command",
            &editor.startup_command,
            |value| Message::ProfileFieldChanged(ProfileField::StartupCommand, value),
        ))
        .push(
            row![
                button("Close")
                    .style(light_button_style)
                    .on_press(Message::CloseDrawer),
                button("Save")
                    .style(dark_button_style)
                    .on_press(Message::SaveProfile),
            ]
            .spacing(10),
        );

    drawer_shell("Edit Profile", content)
}

fn key_drawer(editor: &KeyEditor) -> iced::widget::Container<'_, Message> {
    drawer_shell(
        "Edit Key",
        column![
            labeled_input("Name", &editor.name, |value| {
                Message::KeyFieldChanged(KeyField::Name, value)
            }),
            labeled_text_editor(
                "Private Key",
                &editor.private_key_content,
                "Paste private key",
                |action| { Message::KeyEditorAction(KeyTextField::PrivateKey, action) },
            ),
            labeled_text_editor(
                "Public Key",
                &editor.public_key_content,
                "Paste public key",
                |action| { Message::KeyEditorAction(KeyTextField::PublicKey, action) },
            ),
            labeled_text_editor(
                "Certificate",
                &editor.certificate_content,
                "Paste SSH certificate",
                |action| { Message::KeyEditorAction(KeyTextField::Certificate, action) },
            ),
            row![
                button("Close")
                    .style(light_button_style)
                    .on_press(Message::CloseDrawer),
                button("Save")
                    .style(dark_button_style)
                    .on_press(Message::SaveKey),
            ]
            .spacing(10),
        ]
        .spacing(10),
    )
}

fn identity_drawer<'a>(
    app: &'a App,
    editor: &'a IdentityEditor,
) -> iced::widget::Container<'a, Message> {
    let key_options = std::iter::once("None".to_string())
        .chain(app.keys.iter().map(|key| key.id.to_string()))
        .collect::<Vec<_>>();
    let selected_key = key_options
        .iter()
        .find(|option| option.as_str() == editor.key_id.as_str())
        .cloned()
        .or_else(|| key_options.first().cloned());

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
            labeled_pick_list_owned(
                "Key ID",
                key_options,
                selected_key,
                "Select a key",
                |value| { Message::IdentityFieldChanged(IdentityField::KeyId, value) }
            ),
            row![
                button("Close")
                    .style(light_button_style)
                    .on_press(Message::CloseDrawer),
                button("Save")
                    .style(dark_button_style)
                    .on_press(Message::SaveIdentity),
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
        scrollable(
            column![
                text("Editor")
                    .size(12)
                    .font(iced::Font::MONOSPACE)
                    .color(color_text_muted()),
                text(title).size(28).color(color_text_primary()),
                content
            ]
            .spacing(14),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::default(),
        )),
    )
    .padding(20)
}

fn menu_buttons(app: &App) -> iced::widget::Column<'_, Message> {
    ManageMenu::ALL
        .into_iter()
        .fold(column![].spacing(4), |column, item| {
            column.push(
                button(
                    row![
                        manage_menu_icon(
                            item,
                            if app.selected_menu == item {
                                color_focus()
                            } else {
                                manage_glass_text_secondary()
                            }
                        ),
                        text(item.title())
                            .size(14)
                            .color(if app.selected_menu == item {
                                color_focus()
                            } else {
                                manage_glass_text_secondary()
                            }),
                    ]
                    .spacing(12)
                    .align_y(Vertical::Center),
                )
                .style(move |theme, status| {
                    floating_menu_button_style(app.selected_menu == item, theme, status)
                })
                .padding([10, 14])
                .width(Length::Fill)
                .on_press(Message::SelectMenu(item)),
            )
        })
}

fn terminal_tab_buttons<'a>(
    app: &'a App,
    terminal_theme: Option<TerminalTheme>,
) -> iced::widget::Row<'a, Message> {
    let floating = app.active_tab == ActiveTab::Manage;
    let row = app
        .terminal_tabs
        .iter()
        .fold(row![].spacing(6), |row, tab| {
            let active = app.active_tab == ActiveTab::Terminal(tab.id);
            let label_theme = terminal_theme.clone();
            let badge_theme = terminal_theme.clone();
            let button_theme = terminal_theme.clone();
            let close_theme = terminal_theme.clone();
            let container_theme = terminal_theme.clone();
            let title_max_width = (tab.titlebar_width - 66.0).max(32.0);
            let display_title = truncate_to_width(&tab.title, title_max_width, 13.0);
            let mode = if tab.status.eq_ignore_ascii_case("connecting") {
                "…"
            } else if tab.theme_id == "sftp" {
                "S"
            } else if tab.profile_type == ProfileType::Local {
                "L"
            } else {
                "T"
            };

            row.push(
                container(
                    row![
                        button(
                            row![
                                text(display_title).size(13).width(Length::Fill).color(
                                    if let Some(theme) = label_theme.as_ref() {
                                        terminal_chrome_text_color(theme, active)
                                    } else if active {
                                        color_text_primary()
                                    } else {
                                        color_text_secondary()
                                    }
                                ),
                                container(text(mode).size(8).font(iced::Font::MONOSPACE).color(
                                    if let Some(theme) = badge_theme.as_ref() {
                                        terminal_chrome_muted_color(theme)
                                    } else if active {
                                        color_text_primary()
                                    } else {
                                        color_text_muted()
                                    }
                                ),)
                                .padding([2, 5])
                                .style(move |_| {
                                    if let Some(theme) = badge_theme.as_ref() {
                                        terminal_titlebar_mode_badge_style(active, theme)
                                    } else {
                                        titlebar_mode_badge_style(active, floating)
                                    }
                                }),
                            ]
                            .spacing(8)
                            .align_y(Vertical::Center),
                        )
                        .width(Length::Fill)
                        .style(move |theme_ctx, status| {
                            if let Some(theme) = button_theme.as_ref() {
                                terminal_titlebar_tab_button_style(active, theme, theme_ctx, status)
                            } else {
                                titlebar_tab_button_style(active, floating, theme_ctx, status)
                            }
                        })
                        .on_press(Message::ActivateTerminal(tab.id)),
                        button("×")
                            .style(move |theme_ctx, status| {
                                if let Some(theme) = close_theme.as_ref() {
                                    terminal_titlebar_close_button_style(theme, theme_ctx, status)
                                } else {
                                    titlebar_close_button_style(theme_ctx, status)
                                }
                            })
                            .on_press(Message::CloseTerminal(tab.id)),
                    ]
                    .spacing(4)
                    .align_y(Vertical::Center),
                )
                .width(Length::Fixed(tab.titlebar_width))
                .height(Length::Fixed(28.0))
                .style(move |_| {
                    if container_theme.is_some() {
                        terminal_titlebar_tab_container_style(active)
                    } else {
                        titlebar_tab_container_style(active, floating)
                    }
                }),
            )
        });

    row![
        scrollable(row)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::default()
            ))
            .width(Length::Fill)
    ]
}

fn titlebar_controls<'a>(
    app: &'a App,
    terminal_theme: Option<TerminalTheme>,
) -> iced::widget::Row<'a, Message> {
    let settings = button("Settings")
        .style(move |theme_ctx, status| {
            if let Some(theme) = terminal_theme.as_ref() {
                terminal_titlebar_tab_button_style(false, theme, theme_ctx, status)
            } else if app.active_tab == ActiveTab::Manage {
                floating_menu_button_style(false, theme_ctx, status)
            } else {
                light_button_style(theme_ctx, status)
            }
        })
        .on_press(Message::OpenSettingsWindow);

    row![settings].spacing(8)
}

fn mac_titlebar_spacer<'a>() -> iced::widget::Container<'a, Message> {
    container(Space::new().width(Length::Fixed(MAC_TRAFFIC_LIGHT_SPACER_WIDTH)))
        .width(Length::Fixed(MAC_TRAFFIC_LIGHT_SPACER_WIDTH))
}

fn ansi_color_inputs<'a>(
    normal: &'a [String; 8],
    bright: &'a [String; 8],
) -> iced::widget::Column<'a, Message> {
    const NORMAL_LABELS: [&str; 8] = [
        "normal.black",
        "normal.red",
        "normal.green",
        "normal.yellow",
        "normal.blue",
        "normal.magenta",
        "normal.cyan",
        "normal.white",
    ];
    const BRIGHT_LABELS: [&str; 8] = [
        "bright.black",
        "bright.red",
        "bright.green",
        "bright.yellow",
        "bright.blue",
        "bright.magenta",
        "bright.cyan",
        "bright.white",
    ];

    let normal_inputs =
        normal
            .iter()
            .enumerate()
            .fold(column![].spacing(8), |column, (index, value)| {
                column.push(labeled_input(NORMAL_LABELS[index], value, move |next| {
                    Message::SettingsColorChanged(ColorField::AnsiNormal(index), next)
                }))
            });

    let bright_inputs =
        bright
            .iter()
            .enumerate()
            .fold(column![].spacing(8), |column, (index, value)| {
                column.push(labeled_input(BRIGHT_LABELS[index], value, move |next| {
                    Message::SettingsColorChanged(ColorField::AnsiBright(index), next)
                }))
            });

    column![
        text("normal")
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        normal_inputs,
        Space::new().height(Length::Fixed(8.0)),
        text("bright")
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        bright_inputs,
    ]
    .spacing(8)
}

fn labeled_input<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'static + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        text_input(label, value)
            .on_input(on_input)
            .padding([10, 12])
            .style(field_style),
    ]
    .spacing(6)
}

fn labeled_pick_list<'a>(
    label: &'a str,
    options: &'a [String],
    selected: Option<&'a String>,
    placeholder: &'a str,
    on_select: impl Fn(String) -> Message + 'a + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        pick_list(options, selected, on_select)
            .placeholder(placeholder)
            .width(Length::Fill)
            .padding([10, 12])
            .style(pick_list_field_style),
    ]
    .spacing(6)
}

fn labeled_pick_list_owned<'a>(
    label: &'a str,
    options: Vec<String>,
    selected: Option<String>,
    placeholder: &'a str,
    on_select: impl Fn(String) -> Message + 'static + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        pick_list(options, selected, on_select)
            .placeholder(placeholder)
            .width(Length::Fill)
            .padding([10, 12])
            .style(pick_list_field_style),
    ]
    .spacing(6)
}

fn labeled_text_editor<'a>(
    label: &'a str,
    content: &'a text_editor::Content,
    placeholder: &'a str,
    on_action: impl Fn(text_editor::Action) -> Message + 'a + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        text_editor(content)
            .placeholder(placeholder)
            .on_action(on_action)
            .height(Length::Fixed(132.0))
            .padding(12)
            .style(text_editor_field_style),
    ]
    .spacing(6)
}

fn settings_section<'a>(
    title: &'a str,
    content: iced::widget::Column<'a, Message>,
) -> iced::widget::Container<'a, Message> {
    container(column![text(title).size(22).color(color_text_primary()), content].spacing(14))
        .padding(18)
        .style(|_| section_surface_style())
}
