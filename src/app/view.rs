use super::*;
use iced::widget::column;

const CONNECTION_GRID_GAP: f32 = 14.0;
const SCROLLBAR_SPACING: f32 = 6.0;
const CONTEXT_MENU_ITEM_HEIGHT: f32 = 31.0;
const CONTEXT_MENU_ITEM_GAP: f32 = 2.0;
const CONTEXT_MENU_SHELL_PADDING: f32 = 3.0;
const PICK_LIST_MENU_MAX_HEIGHT: f32 = 220.0;
const TERMIUS_CARD_WIDTH: f32 = 244.0;
const TERMIUS_CARD_HEIGHT: f32 = 60.0;

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
            (_, iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers))) => Some(
                Message::KeyboardInput(window, keyboard::Event::ModifiersChanged(modifiers)),
            ),
            (_, iced::Event::Keyboard(key_event))
                if matches!(
                    &key_event,
                    keyboard::Event::KeyPressed { key, modifiers, .. }
                        if is_copy_shortcut(key, *modifiers) || is_paste_shortcut(key, *modifiers)
                ) =>
            {
                Some(Message::KeyboardInput(window, key_event))
            }
            (event::Status::Ignored, iced::Event::Keyboard(key_event)) => {
                Some(Message::KeyboardInput(window, key_event))
            }
            (event::Status::Ignored, iced::Event::InputMethod(ime_event)) => {
                Some(Message::InputMethod(window, ime_event))
            }
            (_, iced::Event::Mouse(mouse::Event::CursorMoved { position })) => {
                Some(Message::CursorMoved(position))
            }
            (
                event::Status::Ignored,
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            ) => Some(Message::CloseDrawer),
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
        .padding(if manage_active { 0 } else { 1 })
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
    let manage =
        container(
            button(
                row![text("Workspace").size(13).color(
                    if let Some(theme) = terminal_theme.as_ref() {
                        terminal_chrome_text_color(theme, app.active_tab == ActiveTab::Manage)
                    } else if app.active_tab == ActiveTab::Manage {
                        manage_glass_text_primary()
                    } else {
                        manage_glass_text_secondary()
                    }
                ),]
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
    let cursor_shape_options = vec![
        "block".to_string(),
        "beam".to_string(),
        "underline".to_string(),
    ];
    let selected_cursor_shape = cursor_shape_options
        .iter()
        .find(|shape| *shape == &settings.cursor_shape)
        .cloned();

    let appearance_section = settings_window_section(
        ManageMenu::Settings,
        "APPEARANCE",
        column![
            settings_window_row(
                "Terminal Typography",
                "",
                column![
                    settings_window_two_column_fields(
                        settings_window_pick_list(
                            "Terminal Font",
                            app.available_fonts.as_slice(),
                            selected_font,
                            "Select a terminal font",
                            |value| Message::SettingsFontChanged(FontField::Family, value)
                        )
                        .into(),
                        settings_window_input("Font Size", &settings.font_size, |value| {
                            Message::SettingsFontChanged(FontField::Size, value)
                        })
                        .into(),
                    ),
                    settings_window_two_column_fields(
                        settings_window_input("Line Height", &settings.line_height, |value| {
                            Message::SettingsFontChanged(FontField::LineHeight, value)
                        })
                        .into(),
                        settings_window_input(
                            "Scrollback Lines",
                            &settings.scrollback_lines,
                            Message::SettingsScrollbackChanged
                        )
                        .into(),
                    ),
                ]
                .spacing(14)
                .into()
            ),
            settings_window_row(
                "Font Rendering",
                "",
                settings_window_switch(
                    settings.font_thicken,
                    Message::SettingsFontThickenChanged(!settings.font_thicken),
                )
                .into(),
            ),
        ]
        .spacing(18),
    );

    let cursor_section = settings_window_section(
        ManageMenu::PortForwarding,
        "CURSOR",
        column![
            settings_window_row(
                "Cursor Behavior",
                "",
                column![settings_window_two_column_fields(
                    settings_window_pick_list_owned(
                        "Cursor Shape",
                        cursor_shape_options,
                        selected_cursor_shape,
                        "Select a cursor shape",
                        |value| Message::SettingsCursorChanged(CursorField::Shape, value)
                    )
                    .into(),
                    settings_window_switch_field(
                        "Cursor Blink",
                        settings.cursor_blinking,
                        Message::SettingsCursorBlinkChanged(!settings.cursor_blinking)
                    )
                    .into()
                ),]
                .into()
            ),
            settings_window_row(
                "Cursor Colors",
                "",
                column![settings_window_two_column_fields(
                    settings_window_input("Cursor Color", &settings.cursor_color, |value| {
                        Message::SettingsColorChanged(ColorField::CursorColor, value)
                    })
                    .into(),
                    settings_window_input("Cursor Text", &settings.cursor_text, |value| {
                        Message::SettingsColorChanged(ColorField::CursorText, value)
                    })
                    .into()
                ),]
                .into()
            ),
        ]
        .spacing(18),
    );

    let color_section = settings_window_section(
        ManageMenu::KnownHosts,
        "COLORS",
        column![
            settings_window_row(
                "Base Palette",
                "",
                column![
                    settings_window_two_column_fields(
                        settings_window_input("Background", &settings.background, |value| {
                            Message::SettingsColorChanged(ColorField::Background, value)
                        })
                        .into(),
                        settings_window_input("Foreground", &settings.foreground, |value| {
                            Message::SettingsColorChanged(ColorField::Foreground, value)
                        })
                        .into()
                    ),
                    settings_window_two_column_fields(
                        settings_window_input(
                            "Selection Background",
                            &settings.selection_background,
                            |value| {
                                Message::SettingsColorChanged(
                                    ColorField::SelectionBackground,
                                    value,
                                )
                            }
                        )
                        .into(),
                        settings_window_input(
                            "Selection Foreground",
                            &settings.selection_foreground,
                            |value| {
                                Message::SettingsColorChanged(
                                    ColorField::SelectionForeground,
                                    value,
                                )
                            }
                        )
                        .into()
                    ),
                ]
                .spacing(14)
                .into()
            ),
            settings_window_row(
                "ANSI Palette",
                "",
                ansi_color_inputs(&settings.ansi_normal, &settings.ansi_bright).into(),
            ),
        ]
        .spacing(18),
    );

    let sidebar = settings_window_sidebar();
    let content = container(
        column![
            settings_window_header(),
            scrollable(
                column![
                    settings_window_intro(),
                    appearance_section,
                    cursor_section,
                    color_section,
                    row![
                        button("Reset Theme")
                            .style(light_button_style)
                            .on_press(Message::ResetThemeToAtomOneLight),
                        button("Save Settings")
                            .style(dark_button_style)
                            .on_press(Message::SaveSettings),
                    ]
                    .spacing(10),
                ]
                .spacing(26)
                .padding([24, 24])
                .width(Length::Fill)
            )
            .height(Length::Fill)
            .direction(iced::widget::scrollable::Direction::Vertical(
                embedded_vertical_scrollbar(),
            )),
            settings_window_status_bar(),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(0),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| manage_content_surface_style());

    container(
        row![sidebar, content]
            .height(Length::Fill)
            .width(Length::Fill),
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
    let sidebar = container(
        column![
            sidebar_brand(),
            container(menu_buttons(app)).padding([10, 8]),
            Space::new().height(Length::Fill),
            container(rule::horizontal(1)).style(|_| manage_sidebar_style()),
            sidebar_footer()
        ]
        .spacing(0)
        .padding(0),
    )
    .width(Length::Fixed(SIDEBAR_WIDTH))
    .height(Length::Fill)
    .style(|_| sidebar_surface());

    let content_body: Element<'_, Message> = match app.selected_menu {
        ManageMenu::Connections => connections_view(app),
        ManageMenu::Keychain => keychain_view(app),
        ManageMenu::PortForwarding => port_forwarding_view(app),
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
        .padding(0)
        .style(|_| termius_workspace_style());

    let base: Element<'_, Message> = row![sidebar, content]
        .spacing(0)
        .height(Length::Fill)
        .width(Length::Fill)
        .into();

    let base: Element<'_, Message> = {
        let overlay: Element<'_, Message> = match (app.selected_menu, app.context_menu) {
            (
                ManageMenu::Connections,
                Some(ContextMenuState {
                    target: ContextMenuTarget::Connection(id),
                    position,
                }),
            ) => {
                if let Some(connection) = app
                    .connections
                    .iter()
                    .find(|connection| connection.id == id)
                {
                    let item_count = if connection.connection_type == ConnectionType::Ssh {
                        5
                    } else {
                        4
                    };

                    context_menu_overlay_layer(
                        connection_context_menu(connection),
                        position,
                        app.main_window_size,
                        Message::CloseDrawer,
                        156.0,
                        context_menu_height(item_count),
                        TITLEBAR_HEIGHT,
                    )
                } else {
                    Space::new()
                        .width(Length::Shrink)
                        .height(Length::Shrink)
                        .into()
                }
            }
            (
                ManageMenu::Keychain,
                Some(ContextMenuState {
                    target: ContextMenuTarget::Key(id),
                    position,
                }),
            ) => {
                if let Some(key) = app.keys.iter().find(|key| key.id == id) {
                    context_menu_overlay_layer(
                        key_context_menu(key),
                        position,
                        app.main_window_size,
                        Message::CloseDrawer,
                        156.0,
                        context_menu_height(1),
                        TITLEBAR_HEIGHT,
                    )
                } else {
                    Space::new()
                        .width(Length::Shrink)
                        .height(Length::Shrink)
                        .into()
                }
            }
            (
                ManageMenu::Keychain,
                Some(ContextMenuState {
                    target: ContextMenuTarget::Identity(id),
                    position,
                }),
            ) => {
                if let Some(identity) = app.identities.iter().find(|identity| identity.id == id) {
                    context_menu_overlay_layer(
                        identity_context_menu(identity),
                        position,
                        app.main_window_size,
                        Message::CloseDrawer,
                        156.0,
                        context_menu_height(1),
                        TITLEBAR_HEIGHT,
                    )
                } else {
                    Space::new()
                        .width(Length::Shrink)
                        .height(Length::Shrink)
                        .into()
                }
            }
            _ => Space::new()
                .width(Length::Shrink)
                .height(Length::Shrink)
                .into(),
        };

        stack([base, overlay]).into()
    };

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

fn sidebar_brand<'a>() -> iced::widget::Container<'a, Message> {
    container(Space::new().height(Length::Fixed(8.0))).padding(0)
}

fn sidebar_footer<'a>() -> iced::widget::Container<'a, Message> {
    container(
        row![
            container(
                text("TU")
                    .size(10)
                    .font(ui_font_weight(iced::font::Weight::Bold))
                    .color(color_focus())
            )
            .width(Length::Fixed(28.0))
            .height(Length::Fixed(28.0))
            .center_x(Length::Shrink)
            .center_y(Length::Shrink)
            .style(|_| {
                bordered_surface(
                    Color::from_rgb8(232, 240, 255),
                    999.0,
                    Color::from_rgb8(232, 240, 255),
                    Shadow::default(),
                )
            }),
            column![
                text("Timon User")
                    .size(12)
                    .font(ui_font_weight(iced::font::Weight::Semibold))
                    .color(color_text_primary()),
                text("ADMIN")
                    .size(10)
                    .font(ui_font_weight(iced::font::Weight::Bold))
                    .color(color_text_muted()),
            ]
            .spacing(1),
        ]
        .spacing(12)
        .align_y(Vertical::Center),
    )
    .padding([16, 16])
}

fn settings_window_sidebar<'a>() -> iced::widget::Container<'a, Message> {
    container(
        column![
            sidebar_brand(),
            column![
                settings_window_sidebar_item(ManageMenu::Connections, "Connections", false),
                settings_window_sidebar_item(ManageMenu::Keychain, "Keychain", false),
                settings_window_sidebar_item(ManageMenu::PortForwarding, "Port Forwarding", false,),
                settings_window_sidebar_item(ManageMenu::Settings, "Settings", true),
            ]
            .spacing(6)
            .padding([4, 10]),
            Space::new().height(Length::Fill),
            container(rule::horizontal(1)).style(|_| manage_sidebar_style()),
            sidebar_footer(),
        ]
        .spacing(0),
    )
    .width(Length::Fixed(SIDEBAR_WIDTH))
    .height(Length::Fill)
    .style(|_| sidebar_surface())
}

fn settings_window_sidebar_item<'a>(
    icon_menu: ManageMenu,
    label: &'a str,
    active: bool,
) -> iced::widget::Container<'a, Message> {
    let icon_color = if active {
        color_focus()
    } else {
        Color::from_rgb8(148, 163, 184)
    };

    container(
        row![
            manage_menu_icon(icon_menu, icon_color),
            text(label)
                .size(13)
                .font(ui_font_weight(if active {
                    iced::font::Weight::Bold
                } else {
                    iced::font::Weight::Semibold
                }))
                .color(if active {
                    color_focus()
                } else {
                    Color::from_rgb8(71, 85, 105)
                }),
        ]
        .spacing(12)
        .align_y(Vertical::Center),
    )
    .padding([8.5, 12.0])
    .style(move |_| {
        if active {
            bordered_surface(
                Color::WHITE,
                8.0,
                Color::from_rgb8(216, 222, 230),
                Shadow {
                    color: Color::from_rgba8(15, 23, 42, 0.03),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                },
            )
        } else {
            bordered_surface(
                Color::TRANSPARENT,
                8.0,
                Color::TRANSPARENT,
                Shadow::default(),
            )
        }
    })
}

fn settings_window_header<'a>() -> iced::widget::Container<'a, Message> {
    container(
        row![
            text("Settings")
                .size(14)
                .font(ui_font_weight(iced::font::Weight::Bold))
                .color(color_text_primary()),
            Space::new().width(Length::Fill),
            settings_window_search_shell(),
            button("Save")
                .style(dark_button_style)
                .on_press(Message::SaveSettings),
        ]
        .spacing(12)
        .align_y(Vertical::Center),
    )
    .padding([16, 22])
    .style(|_| manage_content_surface_style())
}

fn settings_window_search_shell<'a>() -> iced::widget::Container<'a, Message> {
    container(
        text("Search settings...")
            .size(13)
            .color(color_text_faint()),
    )
    .width(Length::Fixed(192.0))
    .padding([6, 14])
    .style(|_| bordered_surface(color_canvas(), 8.0, color_ring_soft(), Shadow::default()))
}

fn settings_window_intro<'a>() -> iced::widget::Column<'a, Message> {
    column![
        text("Workspace Configuration")
            .size(22)
            .font(ui_font_weight(iced::font::Weight::Bold))
            .color(color_text_primary()),
        text("Manage terminal appearance, cursor behavior, and color credentials.")
            .size(14)
            .color(color_text_secondary()),
    ]
    .spacing(6)
}

fn settings_window_section<'a>(
    icon_menu: ManageMenu,
    label: &'a str,
    content: iced::widget::Column<'a, Message>,
) -> iced::widget::Column<'a, Message> {
    column![
        row![
            manage_menu_icon(icon_menu, color_focus()),
            text(label)
                .size(11)
                .font(ui_font_weight(iced::font::Weight::Bold))
                .color(color_text_secondary()),
        ]
        .spacing(8)
        .align_y(Vertical::Center),
        rule::horizontal(1),
        content,
    ]
    .spacing(16)
}

fn settings_window_row<'a>(
    title: &'a str,
    description: &'a str,
    control: Element<'a, Message>,
) -> iced::widget::Container<'a, Message> {
    let title_block: Element<'a, Message> = if description.is_empty() {
        text(title)
            .size(15)
            .font(ui_font_weight(iced::font::Weight::Semibold))
            .color(color_text_primary())
            .into()
    } else {
        column![
            text(title)
                .size(15)
                .font(ui_font_weight(iced::font::Weight::Semibold))
                .color(color_text_primary()),
            text(description).size(13).color(color_text_secondary()),
        ]
        .spacing(4)
        .into()
    };

    container(
        row![
            container(title_block).width(Length::FillPortion(2)),
            container(control)
                .width(Length::FillPortion(3))
                .center_y(Length::Shrink),
        ]
        .spacing(18)
        .align_y(Vertical::Center),
    )
}

fn settings_window_two_column_fields<'a>(
    left: Element<'a, Message>,
    right: Element<'a, Message>,
) -> iced::widget::Row<'a, Message> {
    row![
        container(left).width(Length::FillPortion(1)),
        container(right).width(Length::FillPortion(1)),
    ]
    .spacing(14)
}

fn settings_window_input<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'static + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(ui_font_weight(iced::font::Weight::Semibold))
            .color(color_text_secondary()),
        text_input(label, value)
            .on_input(on_input)
            .padding([5, 12])
            .style(field_style),
    ]
    .spacing(8)
    .width(Length::Fill)
}

fn settings_window_pick_list<'a>(
    label: &'a str,
    options: &'a [String],
    selected: Option<&'a String>,
    placeholder: &'a str,
    on_select: impl Fn(String) -> Message + 'a + Copy,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(ui_font_weight(iced::font::Weight::Semibold))
            .color(color_text_secondary()),
        pick_list(options, selected, on_select)
            .placeholder(placeholder)
            .width(Length::Fill)
            .menu_height(Length::Fixed(PICK_LIST_MENU_MAX_HEIGHT))
            .padding([5, 12])
            .style(pick_list_field_style)
            .menu_style(pick_list_menu_style),
    ]
    .spacing(8)
    .width(Length::Fill)
}

fn settings_window_pick_list_owned<'a>(
    label: &'a str,
    options: Vec<String>,
    selected: Option<String>,
    placeholder: &'a str,
    on_select: impl Fn(String) -> Message + 'static,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(ui_font_weight(iced::font::Weight::Semibold))
            .color(color_text_secondary()),
        pick_list(options, selected, on_select)
            .placeholder(placeholder)
            .width(Length::Fill)
            .menu_height(Length::Fixed(PICK_LIST_MENU_MAX_HEIGHT))
            .padding([5, 12])
            .style(pick_list_field_style)
            .menu_style(pick_list_menu_style),
    ]
    .spacing(8)
    .width(Length::Fill)
}

fn settings_window_switch<'a>(
    enabled: bool,
    message: Message,
) -> iced::widget::Button<'a, Message> {
    let thumb = container(
        Space::new()
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0)),
    )
    .style(|_| bordered_surface(Color::WHITE, 999.0, Color::TRANSPARENT, Shadow::default()));

    let track = if enabled {
        row![Space::new().width(Length::Fill), thumb]
    } else {
        row![thumb, Space::new().width(Length::Fill)]
    }
    .width(Length::Fixed(38.0))
    .height(Length::Fixed(20.0))
    .padding([2, 2])
    .align_y(Vertical::Center);

    button(container(track).style(move |_| {
        bordered_surface(
            if enabled {
                Color::from_rgb8(16, 185, 129)
            } else {
                Color::from_rgb8(226, 232, 240)
            },
            999.0,
            Color::TRANSPARENT,
            Shadow::default(),
        )
    }))
    .style(|_, _| button::Style::default())
    .on_press(message)
}

fn settings_window_switch_field<'a>(
    label: &'a str,
    enabled: bool,
    message: Message,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(ui_font_weight(iced::font::Weight::Semibold))
            .color(color_text_secondary()),
        settings_window_switch(enabled, message),
    ]
    .spacing(8)
}

fn settings_window_status_bar<'a>() -> iced::widget::Container<'a, Message> {
    container(
        row![
            row![
                text("•").size(14).color(Color::from_rgb8(16, 185, 129)),
                text("LOCAL CONFIG")
                    .size(11)
                    .font(ui_font_weight(iced::font::Weight::Bold))
                    .color(Color::from_rgb8(16, 185, 129)),
                text("TERMINAL PREFERENCES")
                    .size(11)
                    .color(color_text_muted()),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            Space::new().width(Length::Fill),
            text("Timon Settings").size(11).color(color_text_muted()),
        ]
        .align_y(Vertical::Center),
    )
    .padding([8, 16])
    .style(|_| manage_sidebar_style())
}

fn termius_workspace_style() -> container::Style {
    bordered_surface(
        Color::from_rgb8(237, 241, 242),
        0.0,
        Color::TRANSPARENT,
        Shadow::default(),
    )
}

fn termius_toolbar_style() -> container::Style {
    bordered_surface(
        Color::from_rgb8(229, 237, 240),
        0.0,
        Color::TRANSPARENT,
        Shadow::default(),
    )
}

fn termius_search_style() -> container::Style {
    bordered_surface(
        Color::from_rgb8(239, 246, 248),
        10.0,
        Color::from_rgb8(210, 222, 228),
        Shadow::default(),
    )
}

fn termius_card_style() -> container::Style {
    bordered_surface(
        Color::WHITE,
        14.0,
        Color::from_rgba8(15, 23, 42, 0.04),
        Shadow::default(),
    )
}

fn termius_action_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(98, 102, 126),
        button::Status::Pressed => Color::from_rgb8(76, 80, 102),
        _ => Color::from_rgb8(84, 88, 112),
    };

    button_style_base(background, Color::WHITE, background, Shadow::default())
}

fn termius_icon_button_style(active: bool) -> container::Style {
    bordered_surface(
        if active {
            Color::from_rgb8(41, 204, 231)
        } else {
            Color::from_rgb8(144, 166, 181)
        },
        10.0,
        Color::TRANSPARENT,
        Shadow::default(),
    )
}

fn sidebar_surface() -> container::Style {
    bordered_surface(
        Color::from_rgb8(247, 249, 250),
        0.0,
        Color::from_rgb8(223, 230, 235),
        Shadow::default(),
    )
}

fn sidebar_item_style(
    active: bool,
    progress: f32,
    _theme: &Theme,
    status: button::Status,
) -> button::Style {
    let hover_surface = match status {
        button::Status::Hovered => Color::from_rgb8(237, 243, 246),
        button::Status::Pressed => Color::from_rgb8(226, 235, 239),
        _ => Color::TRANSPARENT,
    };
    let active_surface = if active {
        mix_color(
            Color::TRANSPARENT,
            Color::from_rgb8(231, 238, 242),
            progress,
        )
    } else {
        Color::TRANSPARENT
    };
    let background = if active {
        match status {
            button::Status::Hovered => Color::from_rgb8(231, 238, 242),
            button::Status::Pressed => Color::from_rgb8(224, 233, 238),
            _ => active_surface,
        }
    } else {
        hover_surface
    };
    let border_color = mix_color(
        Color::TRANSPARENT,
        Color::TRANSPARENT,
        if active { progress } else { 0.0 },
    );
    let shadow_progress = 0.0;
    let shadow_color = with_alpha(Color::from_rgb8(15, 23, 42), 0.03 * shadow_progress);

    iced::widget::button::Style {
        background: Some(Background::Color(background)),
        text_color: mix_color(Color::from_rgb8(71, 85, 105), color_focus(), progress),
        border: Border {
            color: border_color,
            width: shadow_progress,
            radius: 6.0.into(),
        },
        shadow: Shadow {
            color: shadow_color,
            offset: Vector::new(0.0, shadow_progress),
            blur_radius: 2.0 * shadow_progress,
        },
        ..Default::default()
    }
}

fn sidebar_item<'a>(
    icon: Element<'a, Message>,
    label: &'a str,
    progress: f32,
    active: bool,
    on_press: Message,
) -> iced::widget::Button<'a, Message> {
    button(
        row![
            icon,
            text(label)
                .size(SIDEBAR_MENU_FONT_SIZE_INACTIVE)
                .line_height(iced::Pixels(SIDEBAR_MENU_FONT_SIZE_INACTIVE))
                .align_y(Vertical::Center)
                .font(ui_font_weight(if active {
                    iced::font::Weight::Bold
                } else {
                    iced::font::Weight::Semibold
                }))
                .color(mix_color(
                    Color::from_rgb8(71, 85, 105),
                    color_focus(),
                    progress,
                )),
        ]
        .spacing(12)
        .align_y(Vertical::Center),
    )
    .padding([8.5, 12.0])
    .width(Length::Fill)
    .style(move |theme, status| sidebar_item_style(active, progress, theme, status))
    .on_press(on_press)
}

fn menu_item_style(
    danger: bool,
    active: bool,
    disabled: bool,
    status: button::Status,
) -> iced::widget::button::Style {
    let text_color = if danger {
        Color::from_rgb8(239, 68, 68)
    } else if active {
        color_focus()
    } else if disabled {
        color_text_muted()
    } else {
        color_text_primary()
    };

    let background = match status {
        _ if active => with_alpha(color_focus(), 0.08),
        _ if disabled => Color::TRANSPARENT,
        button::Status::Hovered => Color::from_rgb8(248, 250, 252),
        button::Status::Pressed => Color::from_rgb8(241, 245, 249),
        _ => Color::TRANSPARENT,
    };

    iced::widget::button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
        ..Default::default()
    }
}

fn menu_item<'a>(
    label: &'a str,
    danger: bool,
    on_press: Message,
) -> iced::widget::Button<'a, Message> {
    button(
        text(label)
            .size(13)
            .font(ui_font_weight(iced::font::Weight::Medium))
            .color(if danger {
                Color::from_rgb8(239, 68, 68)
            } else {
                color_text_primary()
            }),
    )
    .padding([7, 10])
    .height(Length::Fixed(CONTEXT_MENU_ITEM_HEIGHT))
    .width(Length::Fill)
    .style(move |_theme, status| menu_item_style(danger, false, false, status))
    .on_press(on_press)
}

fn context_menu<'a>(
    width: f32,
    content: iced::widget::Column<'a, Message>,
) -> iced::widget::Container<'a, Message> {
    container(content)
        .padding(CONTEXT_MENU_SHELL_PADDING)
        .width(Length::Fixed(width))
        .style(|_| context_menu_style())
}

fn context_menu_height(item_count: usize) -> f32 {
    if item_count == 0 {
        CONTEXT_MENU_SHELL_PADDING * 2.0
    } else {
        CONTEXT_MENU_SHELL_PADDING * 2.0
            + CONTEXT_MENU_ITEM_HEIGHT * item_count as f32
            + CONTEXT_MENU_ITEM_GAP * item_count.saturating_sub(1) as f32
    }
}

fn context_menu_style() -> container::Style {
    bordered_surface(
        Color::from_rgb8(255, 255, 255),
        12.0,
        color_ring_soft(),
        Shadow {
            color: Color::from_rgba8(15, 23, 42, 0.08),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 24.0,
        },
    )
}

fn context_menu_overlay_layer<'a>(
    menu: Element<'a, Message>,
    position: Option<Point>,
    window_size: iced::Size,
    on_close: Message,
    menu_width: f32,
    menu_height_guess: f32,
    top_offset: f32,
) -> Element<'a, Message> {
    let inset = 12.0;
    let cursor_gap = 6.0;
    let safe_menu_height = menu_height_guess;
    let left = position
        .map(|cursor| {
            let cursor_x = cursor.x;
            let fits_right = cursor_x + cursor_gap + menu_width <= window_size.width - inset;
            let fits_left = cursor_x - cursor_gap - menu_width >= inset;

            let proposed = if fits_right || !fits_left {
                cursor_x + cursor_gap
            } else {
                cursor_x - cursor_gap - menu_width
            };

            proposed.clamp(inset, (window_size.width - menu_width - inset).max(inset))
        })
        .unwrap_or(24.0);
    let top = position
        .map(|cursor| {
            let cursor_y = cursor.y - top_offset;
            let fits_bottom =
                cursor_y + cursor_gap + safe_menu_height <= window_size.height - inset;
            let fits_top = cursor_y - cursor_gap - safe_menu_height >= inset;

            let proposed = if fits_bottom || !fits_top {
                cursor_y + cursor_gap
            } else {
                cursor_y - cursor_gap - safe_menu_height
            };

            proposed.clamp(
                inset,
                (window_size.height - safe_menu_height - inset).max(inset),
            )
        })
        .unwrap_or(24.0);

    stack([
        mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(on_close)
        .into(),
        column![
            Space::new().height(Length::Fixed(top)),
            row![Space::new().width(Length::Fixed(left)), menu].align_y(Vertical::Top)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    ])
    .into()
}

fn connection_grid_columns(
    available_width: f32,
    min_width: f32,
    gap: f32,
    max_columns: usize,
) -> usize {
    let max_fit_columns = ((available_width.max(min_width) + gap) / (min_width + gap))
        .floor()
        .max(1.0) as usize;

    max_fit_columns.min(max_columns).max(1)
}

fn connection_grid_item_width(
    available_width: f32,
    columns: usize,
    min_width: f32,
    max_width: f32,
    gap: f32,
) -> f32 {
    ((available_width.max(min_width) - gap * (columns.saturating_sub(1) as f32)) / columns as f32)
        .clamp(min_width, max_width)
}

fn connection_cards_grid<'a>(connections: &'a [Connection]) -> Element<'a, Message> {
    iced::widget::responsive(move |size| {
        let columns =
            connection_grid_columns(size.width, TERMIUS_CARD_WIDTH, CONNECTION_GRID_GAP, 3);
        let item_width = connection_grid_item_width(
            size.width,
            columns,
            TERMIUS_CARD_WIDTH,
            TERMIUS_CARD_WIDTH,
            CONNECTION_GRID_GAP,
        );

        let rows = connections.chunks(columns).fold(
            column![].spacing(CONNECTION_GRID_GAP),
            |column, chunk| {
                let row =
                    chunk
                        .iter()
                        .fold(row![].spacing(CONNECTION_GRID_GAP), |row, connection| {
                            row.push(
                                container(connection_list_card(connection))
                                    .width(Length::Fixed(item_width))
                                    .height(Length::Fixed(TERMIUS_CARD_HEIGHT)),
                            )
                        });

                column.push(row)
            },
        );

        container(rows).width(Length::Fill).into()
    })
    .into()
}

fn embedded_vertical_scrollbar() -> iced::widget::scrollable::Scrollbar {
    iced::widget::scrollable::Scrollbar::default().spacing(SCROLLBAR_SPACING)
}

fn embedded_horizontal_scrollbar() -> iced::widget::scrollable::Scrollbar {
    iced::widget::scrollable::Scrollbar::default().spacing(SCROLLBAR_SPACING)
}

fn terminal_page_view(app: &App, id: u64) -> Element<'_, Message> {
    let Some(tab) = app.terminal_tabs.iter().find(|tab| tab.id == id) else {
        return placeholder_view("Terminal", "Tab not found").into();
    };

    if let TabWorkspace::Sftp(sftp) = &tab.workspace {
        return sftp_page_view(id, tab, sftp).into();
    }

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
        app.keyboard_modifiers.command(),
        Arc::new(move |event| match event {
            TerminalCanvasEvent::SelectionStarted(point) => {
                Message::TerminalSelectionStarted(id, point)
            }
            TerminalCanvasEvent::SelectionUpdated(point) => {
                Message::TerminalSelectionUpdated(id, point)
            }
            TerminalCanvasEvent::SelectionWord(selection) => {
                Message::TerminalSelectionWord(id, selection)
            }
            TerminalCanvasEvent::SelectionToken(selection) => {
                Message::TerminalSelectionToken(id, selection)
            }
            TerminalCanvasEvent::CommandClick(point) => Message::TerminalCommandClick(id, point),
            TerminalCanvasEvent::Scrolled { lines, point } => {
                Message::TerminalScrolled(id, lines, point)
            }
            TerminalCanvasEvent::Resized { cols, rows } => Message::TerminalResized(id, cols, rows),
        }),
    )
    .element();

    let composer_height = app
        .terminal_composer_editor_height(app.terminal_composer_visual_lines(&tab.composer.text()));
    let terminal_theme = app.terminal_theme(&tab.theme_id);
    let composer_editor_theme = terminal_theme.clone();
    let composer_area_theme = terminal_theme.clone();
    let composer_is_multiline = tab.composer.line_count() > 1;
    let composer_content: Element<'_, Message> = suppress_text_editor_ime_preedit(
        text_editor(&tab.composer)
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
                } else if !composer_is_multiline
                    && !keypress.modifiers.shift()
                    && !keypress.modifiers.control()
                    && !keypress.modifiers.alt()
                    && !keypress.modifiers.command()
                    && matches!(
                        keypress.key,
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp)
                    )
                {
                    Some(iced::widget::text_editor::Binding::Custom(
                        Message::TerminalComposerHistoryPrev(id),
                    ))
                } else if !composer_is_multiline
                    && !keypress.modifiers.shift()
                    && !keypress.modifiers.control()
                    && !keypress.modifiers.alt()
                    && !keypress.modifiers.command()
                    && matches!(
                        keypress.key,
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown)
                    )
                {
                    Some(iced::widget::text_editor::Binding::Custom(
                        Message::TerminalComposerHistoryNext(id),
                    ))
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
            .style(move |theme, status| {
                terminal_composer_style(&composer_editor_theme, theme, status)
            }),
    );

    let composer = container(container(composer_content).width(Length::Fill))
        .padding([
            TERMINAL_COMPOSER_PADDING_Y,
            TERMINAL_COMPOSER_HORIZONTAL_PADDING,
        ])
        .style(move |_| terminal_composer_area_style(&composer_area_theme));

    container(column![
        container(terminal).width(Length::Fill).height(Length::Fill),
        composer
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn connections_view(app: &App) -> Element<'_, Message> {
    let search_bar = container(
        container(
            row![
                text("Find a host or ssh user@hostname...")
                    .size(14)
                    .color(Color::from_rgb8(117, 139, 151)),
                Space::new().width(Length::Fill),
                container(
                    text("CONNECT")
                        .size(11)
                        .font(ui_font_weight(iced::font::Weight::Bold))
                        .color(Color::from_rgb8(156, 174, 184))
                )
                .padding([6, 11])
                .style(|_| {
                    bordered_surface(
                        Color::from_rgb8(221, 232, 237),
                        8.0,
                        Color::TRANSPARENT,
                        Shadow::default(),
                    )
                }),
            ]
            .align_y(Vertical::Center),
        )
        .height(Length::Fixed(36.0))
        .width(Length::Fill)
        .padding([0, 10])
        .style(|_| termius_search_style()),
    )
    .height(Length::Fixed(48.0))
    .width(Length::Fill)
    .padding([6, 10])
    .style(|_| termius_toolbar_style());

    let action_bar = container(
        row![
            button("▦  NEW HOST")
                .style(termius_action_button_style)
                .on_press(Message::NewConnection),
            button("⌄")
                .style(termius_action_button_style)
                .on_press(Message::NewGroup),
            button("▣  TERMINAL")
                .style(termius_action_button_style)
                .on_press(Message::NewConnection),
            button("▰  SERIAL")
                .style(termius_action_button_style)
                .on_press(Message::NewSerialConnection),
            Space::new().width(Length::Fill),
            text("▦⌄  ◆⌄  A⌄")
                .size(18)
                .color(Color::from_rgb8(22, 28, 44)),
            container(text("0").size(13).color(Color::WHITE))
                .width(Length::Fixed(28.0))
                .height(Length::Fixed(28.0))
                .center_x(Length::Shrink)
                .center_y(Length::Shrink)
                .style(|_| termius_icon_button_style(true)),
            container(text("+").size(20).color(Color::WHITE))
                .width(Length::Fixed(28.0))
                .height(Length::Fixed(28.0))
                .center_x(Length::Shrink)
                .center_y(Length::Shrink)
                .style(|_| termius_icon_button_style(false)),
        ]
        .spacing(10)
        .align_y(Vertical::Center),
    )
    .height(Length::Fixed(46.0))
    .width(Length::Fill)
    .padding([8, 12])
    .style(|_| termius_toolbar_style());

    let group_cards = if app.groups.is_empty() {
        row![termius_group_card("Default".into(), "0 Hosts".into())]
            .spacing(CONNECTION_GRID_GAP)
            .wrap()
    } else {
        row(app
            .groups
            .iter()
            .filter(|group| group.parent_id.is_none())
            .map(|group| {
                let count = app
                    .connections
                    .iter()
                    .filter(|connection| connection.group_id == Some(group.id))
                    .count();
                termius_group_card(group.name.clone(), format!("{count} Hosts"))
            })
            .collect::<Vec<Element<'_, Message>>>())
        .spacing(CONNECTION_GRID_GAP)
        .wrap()
    };

    let all_connections: Element<'_, Message> = if app.connections.is_empty() {
        container(
            text("No hosts yet.")
                .size(14)
                .color(Color::from_rgb8(77, 91, 105)),
        )
        .padding([18, 20])
        .style(|_| termius_card_style())
        .into()
    } else {
        connection_cards_grid(app.connections.as_slice())
    };

    column![
        search_bar,
        action_bar,
        scrollable(
            column![
                termius_section_title("Groups"),
                group_cards,
                Space::new().height(Length::Fixed(12.0)),
                termius_section_title("Hosts"),
                all_connections,
            ]
            .spacing(14)
            .padding([20, 32])
            .width(Length::Fill)
        )
        .height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            embedded_vertical_scrollbar(),
        )),
    ]
    .spacing(0)
    .height(Length::Fill)
    .into()
}

fn termius_section_title<'a>(label: &'a str) -> Element<'a, Message> {
    text(label)
        .size(15)
        .font(ui_font_weight(iced::font::Weight::Bold))
        .color(Color::from_rgb8(18, 25, 39))
        .into()
}

fn termius_group_card(name: String, caption: String) -> Element<'static, Message> {
    container(
        row![
            container(text("▦").size(22).color(Color::WHITE))
                .width(Length::Fixed(40.0))
                .height(Length::Fixed(40.0))
                .center_x(Length::Shrink)
                .center_y(Length::Shrink)
                .style(|_| {
                    bordered_surface(
                        Color::from_rgb8(0, 91, 139),
                        10.0,
                        Color::TRANSPARENT,
                        Shadow::default(),
                    )
                }),
            column![
                text(name)
                    .size(14)
                    .font(ui_font_weight(iced::font::Weight::Semibold))
                    .color(Color::from_rgb8(20, 28, 39)),
                text(caption)
                    .size(12)
                    .color(Color::from_rgb8(116, 134, 148)),
            ]
            .spacing(1),
        ]
        .spacing(14)
        .align_y(Vertical::Center),
    )
    .padding([10, 12])
    .width(Length::Fixed(244.0))
    .height(Length::Fixed(60.0))
    .style(|_| termius_card_style())
    .into()
}

fn connection_avatar(
    connection: &Connection,
    size: f32,
) -> iced::widget::Container<'static, Message> {
    let accent = match connection.connection_type {
        ConnectionType::Local => Color::from_rgb8(50, 211, 142),
        ConnectionType::Ssh => Color::from_rgb8(242, 78, 28),
        ConnectionType::Serial => Color::from_rgb8(87, 106, 255),
    };

    container(connection_type_icon(
        connection.connection_type,
        Color::WHITE,
        size * 0.46,
    ))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Shrink)
    .center_y(Length::Shrink)
    .style(move |_| bordered_surface(accent, 10.0, Color::TRANSPARENT, Shadow::default()))
}

fn connection_list_card<'a>(connection: &'a Connection) -> Element<'a, Message> {
    mouse_area(
        container(
            row![
                connection_avatar(connection, 40.0),
                column![
                    text(&connection.name)
                        .size(13)
                        .font(ui_font_weight(iced::font::Weight::Semibold))
                        .color(Color::from_rgb8(20, 28, 39)),
                    text(connection_termius_secondary_text(connection))
                        .size(12)
                        .color(Color::from_rgb8(96, 114, 128)),
                ]
                .spacing(1)
                .width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Vertical::Center),
        )
        .padding([10, 12])
        .style(|_| termius_card_style()),
    )
    .on_right_press(Message::OpenConnectionContext(connection.id))
    .into()
}

fn connection_termius_secondary_text(connection: &Connection) -> String {
    match connection.connection_type {
        ConnectionType::Local => "local".to_string(),
        ConnectionType::Serial => {
            if connection.serial_port.trim().is_empty() {
                format!("serial, {}", connection.baud_rate)
            } else {
                format!(
                    "serial, {} @ {}",
                    connection.serial_port, connection.baud_rate
                )
            }
        }
        ConnectionType::Ssh => {
            if !connection.display_username.trim().is_empty() {
                format!("ssh, {}", connection.display_username)
            } else if !connection.username.trim().is_empty() {
                format!("ssh, {}", connection.username)
            } else {
                "ssh".to_string()
            }
        }
    }
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
        |column, key| column.push(key_card(key)),
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
        |column, identity| column.push(identity_card(identity)),
    );

    scrollable(column![keys, identities].spacing(24))
        .height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            embedded_vertical_scrollbar(),
        ))
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
    .direction(iced::widget::scrollable::Direction::Vertical(
        embedded_vertical_scrollbar(),
    ))
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
    .direction(iced::widget::scrollable::Direction::Vertical(
        embedded_vertical_scrollbar(),
    ))
    .into()
}

fn port_forwarding_view(app: &App) -> Element<'_, Message> {
    let content = app.port_forwards.iter().fold(
        column![
            row![
                column![
                    text("Port Forwarding").size(32).color(color_text_primary()),
                    text("Local tunnel rules backed by SSH connections.")
                        .size(14)
                        .color(color_text_secondary()),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                button("New Forward")
                    .style(dark_button_style)
                    .on_press(Message::NewPortForward),
            ]
            .align_y(Vertical::Center)
        ]
        .spacing(12),
        |column, forward| column.push(port_forward_card(app, forward)),
    );

    scrollable(content)
        .height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            embedded_vertical_scrollbar(),
        ))
        .into()
}

fn sftp_page_view<'a>(
    id: u64,
    tab: &'a TerminalTab,
    sftp: &'a SftpTabState,
) -> iced::widget::Container<'a, Message> {
    let entries: Element<'a, Message> = if sftp.entries.is_empty() && !sftp.loading {
        container(text("No entries").size(14).color(color_text_secondary())).into()
    } else {
        scrollable(
            column(
                sftp.entries
                    .iter()
                    .map(|entry| {
                        button(
                            row![
                                text(if entry.is_dir { "DIR" } else { "FILE" })
                                    .size(10)
                                    .font(iced::Font::MONOSPACE)
                                    .color(color_text_muted()),
                                text(entry.name.clone())
                                    .size(14)
                                    .color(color_text_primary()),
                                Space::new().width(Length::Fill),
                                text(if entry.is_dir {
                                    "-".to_string()
                                } else {
                                    format_size(entry.size)
                                })
                                .size(12)
                                .font(iced::Font::MONOSPACE)
                                .color(color_text_muted()),
                            ]
                            .spacing(10)
                            .align_y(Vertical::Center),
                        )
                        .style(light_button_style)
                        .width(Length::Fill)
                        .on_press(Message::SftpOpenEntry(id, entry.path.clone(), entry.is_dir))
                        .into()
                    })
                    .collect::<Vec<Element<'a, Message>>>(),
            )
            .spacing(6),
        )
        .direction(iced::widget::scrollable::Direction::Vertical(
            embedded_vertical_scrollbar(),
        ))
        .into()
    };

    let preview = scrollable(
        container(
            text(if sftp.preview.is_empty() {
                "Select a file to preview"
            } else {
                &sftp.preview
            })
            .font(iced::Font::MONOSPACE)
            .size(13)
            .color(color_text_primary()),
        )
        .padding(14)
        .style(|_| section_surface_style()),
    )
    .direction(iced::widget::scrollable::Direction::Vertical(
        embedded_vertical_scrollbar(),
    ));

    container(
        column![
            row![
                column![
                    text("SFTP").size(28).color(color_text_primary()),
                    text(&sftp.current_path)
                        .size(13)
                        .font(iced::Font::MONOSPACE)
                        .color(color_text_secondary()),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                button("Up")
                    .style(light_button_style)
                    .on_press(Message::SftpOpenParent(id)),
                button("Refresh")
                    .style(light_button_style)
                    .on_press(Message::SftpRefresh(id)),
            ]
            .align_y(Vertical::Center),
            row![
                container(entries)
                    .width(Length::FillPortion(3))
                    .height(Length::Fill)
                    .style(|_| section_surface_style())
                    .padding(12),
                container(preview)
                    .width(Length::FillPortion(2))
                    .height(Length::Fill),
            ]
            .spacing(16)
            .height(Length::Fill),
            text(&tab.status).size(12).color(color_text_muted()),
        ]
        .spacing(14),
    )
    .padding(18)
    .width(Length::Fill)
    .height(Length::Fill)
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
        DrawerState::Connection(editor) => connection_drawer(app, editor).into(),
        DrawerState::Group(editor) => group_drawer(app, editor).into(),
        DrawerState::Key(editor) => key_drawer(editor).into(),
        DrawerState::Identity(editor) => identity_drawer(app, editor).into(),
        DrawerState::PortForward(editor) => port_forward_drawer(app, editor).into(),
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

fn connection_context_menu(connection: &Connection) -> Element<'static, Message> {
    let mut actions = column![menu_item(
        "Connect",
        false,
        Message::ConnectConnection(connection.id),
    )]
    .spacing(CONTEXT_MENU_ITEM_GAP);

    if connection.connection_type == ConnectionType::Ssh {
        actions = actions.push(menu_item(
            "Open SFTP",
            false,
            Message::OpenSftpConnection(connection.id),
        ));
    }

    actions = actions
        .push(menu_item(
            "Duplicate",
            false,
            Message::DuplicateConnection(connection.id),
        ))
        .push(menu_item(
            "Edit",
            false,
            Message::EditConnection(connection.id),
        ))
        .push(menu_item(
            "Delete",
            true,
            Message::DeleteConnection(connection.id),
        ));

    context_menu(156.0, actions).into()
}

fn key_context_menu(key: &SshKey) -> Element<'static, Message> {
    context_menu(
        156.0,
        column![menu_item("Edit", false, Message::EditKey(key.id))].spacing(CONTEXT_MENU_ITEM_GAP),
    )
    .into()
}

fn identity_context_menu(identity: &Identity) -> Element<'static, Message> {
    context_menu(
        156.0,
        column![menu_item("Edit", false, Message::EditIdentity(identity.id))]
            .spacing(CONTEXT_MENU_ITEM_GAP),
    )
    .into()
}

fn key_card<'a>(key: &'a SshKey) -> Element<'a, Message> {
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
    ]
    .spacing(12);

    mouse_area(container(body).padding(18).style(|_| card_surface_style()))
        .on_right_press(Message::OpenKeyContext(key.id))
        .into()
}

fn identity_card<'a>(identity: &'a Identity) -> Element<'a, Message> {
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
    ]
    .spacing(12);

    mouse_area(container(body).padding(18).style(|_| card_surface_style()))
        .on_right_press(Message::OpenIdentityContext(identity.id))
        .into()
}

fn connection_drawer<'a>(
    app: &'a App,
    editor: &'a ConnectionEditor,
) -> iced::widget::Container<'a, Message> {
    let type_options = vec!["ssh".to_string(), "local".to_string(), "serial".to_string()];
    let group_option_pairs = std::iter::once(("None".to_string(), "None".to_string()))
        .chain(
            app.groups
                .iter()
                .map(|group| (group_display_name(app, group), group.id.to_string())),
        )
        .collect::<Vec<_>>();
    let group_options = group_option_pairs
        .iter()
        .map(|(label, _)| label.clone())
        .collect::<Vec<_>>();
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
        .find(|option| option.as_str() == editor.connection_type.as_str())
        .cloned();
    let selected_group = editor
        .group_id
        .trim()
        .parse::<i64>()
        .ok()
        .and_then(|group_id| {
            app.groups
                .iter()
                .find(|group| group.id == group_id)
                .map(|group| group_display_name(app, group))
        })
        .or_else(|| group_options.first().cloned());
    let group_option_pairs_for_input = group_option_pairs.clone();
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
            |value| Message::ConnectionFieldChanged(ConnectionField::KeyId, value),
        )
        .into(),
        labeled_pick_list_owned(
            "Identity ID",
            identity_options,
            selected_identity,
            "Select an identity",
            |value| Message::ConnectionFieldChanged(ConnectionField::IdentityId, value),
        )
        .into(),
        labeled_input("Host", &editor.host, |value| {
            Message::ConnectionFieldChanged(ConnectionField::Host, value)
        })
        .into(),
        labeled_input("Port", &editor.port, |value| {
            Message::ConnectionFieldChanged(ConnectionField::Port, value)
        })
        .into(),
        labeled_input("Username", &editor.username, |value| {
            Message::ConnectionFieldChanged(ConnectionField::Username, value)
        })
        .into(),
        labeled_input("Password", &editor.password, |value| {
            Message::ConnectionFieldChanged(ConnectionField::Password, value)
        })
        .into(),
    ];
    let local_fields: Vec<Element<'a, Message>> = vec![
        labeled_pick_list_owned(
            "Shell",
            shell_options,
            selected_shell,
            "Select a shell",
            |value| Message::ConnectionFieldChanged(ConnectionField::ShellPath, value),
        )
        .into(),
        labeled_input("Work Dir", &editor.work_dir, |value| {
            Message::ConnectionFieldChanged(ConnectionField::WorkDir, value)
        })
        .into(),
    ];
    let serial_fields: Vec<Element<'a, Message>> = vec![
        labeled_input("Serial Port", &editor.serial_port, |value| {
            Message::ConnectionFieldChanged(ConnectionField::SerialPort, value)
        })
        .into(),
        labeled_input("Baud Rate", &editor.baud_rate, |value| {
            Message::ConnectionFieldChanged(ConnectionField::BaudRate, value)
        })
        .into(),
    ];

    let mut content = column![
        labeled_input("Name", &editor.name, |value| {
            Message::ConnectionFieldChanged(ConnectionField::Name, value)
        }),
        labeled_pick_list_owned(
            "Type",
            type_options,
            selected_type,
            "Select a type",
            |value| { Message::ConnectionTypeChanged(value) }
        ),
        labeled_pick_list_owned(
            "Group",
            group_options,
            selected_group,
            "Select a group",
            move |value| {
                let next = group_option_pairs_for_input
                    .iter()
                    .find(|(label, _)| *label == value)
                    .map(|(_, id)| id.clone())
                    .unwrap_or_else(|| "None".to_string());
                Message::ConnectionFieldChanged(ConnectionField::GroupId, next)
            },
        ),
    ]
    .spacing(10);

    let section_fields = match editor.connection_type {
        ConnectionType::Local => local_fields,
        ConnectionType::Serial => serial_fields,
        ConnectionType::Ssh => ssh_fields,
    };

    for field in section_fields {
        content = content.push(field);
    }

    content = content
        .push(theme_gallery(app, editor))
        .push(labeled_input(
            "Startup Command",
            &editor.startup_command,
            |value| Message::ConnectionFieldChanged(ConnectionField::StartupCommand, value),
        ))
        .push(
            row![
                button("Close")
                    .style(light_button_style)
                    .on_press(Message::CloseDrawer),
                button("Save")
                    .style(dark_button_style)
                    .on_press(Message::SaveConnection),
            ]
            .spacing(10),
        );

    drawer_shell("Edit Connection", content)
}

fn theme_gallery<'a>(app: &'a App, editor: &'a ConnectionEditor) -> Element<'a, Message> {
    let themes = std::iter::once((
        "default".to_string(),
        "settings.json".to_string(),
        [
            "#fafafa".to_string(),
            "#383a42".to_string(),
            "#4078f2".to_string(),
        ],
    ))
    .chain(app.terminal_themes.iter().map(|theme| {
        (
            theme.id.clone(),
            theme.path.display().to_string(),
            [
                theme.colors.primary.background.clone(),
                theme.colors.primary.foreground.clone(),
                theme.colors.cursor.cursor.clone(),
            ],
        )
    }))
    .collect::<Vec<_>>();

    column![
        text("Theme")
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        row(themes
            .into_iter()
            .map(|(id, path, colors)| {
                let active = editor.theme_id == id;
                let label = id.clone();
                button(
                    column![
                        text(label).size(13).color(if active {
                            color_focus()
                        } else {
                            color_text_primary()
                        }),
                        text(path)
                            .size(10)
                            .font(iced::Font::MONOSPACE)
                            .color(color_text_muted()),
                        row(colors
                            .into_iter()
                            .map(|hex| {
                                container(
                                    Space::new()
                                        .width(Length::Fixed(12.0))
                                        .height(Length::Fixed(12.0)),
                                )
                                .style(move |_| {
                                    bordered_surface(
                                        parse_hex_color(&hex).unwrap_or(Color::BLACK),
                                        999.0,
                                        Color::TRANSPARENT,
                                        Shadow::default(),
                                    )
                                })
                                .into()
                            })
                            .collect::<Vec<Element<'_, Message>>>())
                        .spacing(6),
                    ]
                    .spacing(6),
                )
                .padding([10, 12])
                .style(move |theme, status| {
                    if active {
                        floating_menu_button_style(true, theme, status)
                    } else {
                        light_button_style(theme, status)
                    }
                })
                .on_press(Message::ConnectionFieldChanged(
                    ConnectionField::ThemeId,
                    id,
                ))
                .into()
            })
            .collect::<Vec<Element<'_, Message>>>())
        .spacing(10)
        .wrap(),
    ]
    .spacing(8)
    .into()
}

fn group_drawer<'a>(app: &'a App, editor: &'a GroupEditor) -> iced::widget::Container<'a, Message> {
    let parent_option_pairs = std::iter::once(("None".to_string(), "None".to_string()))
        .chain(
            app.groups
                .iter()
                .filter(|group| group.parent_id.is_none() && Some(group.id) != editor.id)
                .map(|group| (group.name.clone(), group.id.to_string())),
        )
        .collect::<Vec<_>>();
    let parent_options = parent_option_pairs
        .iter()
        .map(|(label, _)| label.clone())
        .collect::<Vec<_>>();
    let selected_parent = editor
        .parent_id
        .trim()
        .parse::<i64>()
        .ok()
        .and_then(|parent_id| {
            app.groups
                .iter()
                .find(|group| group.id == parent_id)
                .map(|group| group.name.clone())
        })
        .or_else(|| parent_options.first().cloned());
    let parent_option_pairs_for_input = parent_option_pairs.clone();
    drawer_shell(
        "Edit Group",
        column![
            labeled_input("Name", &editor.name, |value| {
                Message::GroupFieldChanged(GroupField::Name, value)
            }),
            labeled_pick_list_owned(
                "Parent Group",
                parent_options,
                selected_parent,
                "Optional parent group",
                move |value| {
                    let next = parent_option_pairs_for_input
                        .iter()
                        .find(|(label, _)| *label == value)
                        .map(|(_, id)| id.clone())
                        .unwrap_or_else(|| "None".to_string());
                    Message::GroupFieldChanged(GroupField::ParentId, next)
                },
            ),
            row![
                button("Close")
                    .style(light_button_style)
                    .on_press(Message::CloseDrawer),
                button("Save")
                    .style(dark_button_style)
                    .on_press(Message::SaveGroup),
            ]
            .spacing(10),
        ]
        .spacing(10),
    )
}

fn group_display_name(app: &App, group: &Group) -> String {
    if let Some(parent_id) = group.parent_id {
        if let Some(parent) = app
            .groups
            .iter()
            .find(|candidate| candidate.id == parent_id)
        {
            return format!("{} / {}", parent.name, group.name);
        }
    }

    group.name.clone()
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

fn port_forward_card<'a>(app: &'a App, forward: &'a PortForward) -> Element<'a, Message> {
    let running = app.port_forward_runtimes.contains_key(&forward.id) || forward.enabled;
    container(
        column![
            row![
                column![
                    text(if forward.label.trim().is_empty() {
                        "Forward"
                    } else {
                        &forward.label
                    })
                    .size(18)
                    .color(color_text_primary()),
                    text(format!(
                        "{}:{} → {}:{}",
                        forward.bind_address,
                        forward.bind_port,
                        forward.destination_host,
                        forward.destination_port
                    ))
                    .size(13)
                    .font(iced::Font::MONOSPACE)
                    .color(color_text_secondary()),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                button(if running { "Stop" } else { "Start" })
                    .style(if running {
                        light_button_style
                    } else {
                        dark_button_style
                    })
                    .on_press(Message::TogglePortForward(forward.id, !running)),
                button("Edit")
                    .style(light_button_style)
                    .on_press(Message::EditPortForward(forward.id)),
                button("Delete")
                    .style(light_button_style)
                    .on_press(Message::DeletePortForward(forward.id)),
            ]
            .align_y(Vertical::Center),
            text(format!(
                "{} via {}",
                forward.forward_type.label(),
                if forward.connection_name.trim().is_empty() {
                    "Unassigned SSH Connection"
                } else {
                    &forward.connection_name
                }
            ))
            .size(12)
            .color(color_text_muted()),
        ]
        .spacing(10),
    )
    .padding(16)
    .style(|_| card_surface_style())
    .into()
}

fn port_forward_drawer<'a>(
    app: &'a App,
    editor: &'a PortForwardEditor,
) -> iced::widget::Container<'a, Message> {
    let type_options = vec![
        "local".to_string(),
        "remote".to_string(),
        "dynamic".to_string(),
    ];
    let connection_options = std::iter::once("None".to_string())
        .chain(
            app.connections
                .iter()
                .filter(|connection| connection.connection_type == ConnectionType::Ssh)
                .map(|connection| connection.id.to_string()),
        )
        .collect::<Vec<_>>();
    let selected_type = type_options
        .iter()
        .find(|option| option.as_str() == editor.forward_type.as_str())
        .cloned();
    let selected_connection = connection_options
        .iter()
        .find(|option| option.as_str() == editor.connection_id.as_str())
        .cloned()
        .or_else(|| connection_options.first().cloned());

    drawer_shell(
        "Edit Port Forward",
        column![
            labeled_input("Label", &editor.label, |value| {
                Message::PortForwardFieldChanged(PortForwardField::Label, value)
            }),
            labeled_pick_list_owned(
                "Type",
                type_options,
                selected_type,
                "Select a type",
                |value| { Message::PortForwardTypeChanged(value) }
            ),
            labeled_input("Bind Address", &editor.bind_address, |value| {
                Message::PortForwardFieldChanged(PortForwardField::BindAddress, value)
            }),
            labeled_input("Bind Port", &editor.bind_port, |value| {
                Message::PortForwardFieldChanged(PortForwardField::BindPort, value)
            }),
            labeled_pick_list_owned(
                "SSH Connection",
                connection_options,
                selected_connection,
                "Select a connection",
                |value| Message::PortForwardFieldChanged(PortForwardField::ConnectionId, value),
            ),
            labeled_input("Destination Host", &editor.destination_host, |value| {
                Message::PortForwardFieldChanged(PortForwardField::DestinationHost, value)
            }),
            labeled_input("Destination Port", &editor.destination_port, |value| {
                Message::PortForwardFieldChanged(PortForwardField::DestinationPort, value)
            }),
            row![
                button("Close")
                    .style(light_button_style)
                    .on_press(Message::CloseDrawer),
                button("Save")
                    .style(dark_button_style)
                    .on_press(Message::SavePortForward),
            ]
            .spacing(10),
        ]
        .spacing(10),
    )
}

fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{size} B")
    }
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::from_rgb8(red, green, blue))
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
            embedded_vertical_scrollbar(),
        )),
    )
    .padding(20)
}

fn menu_buttons(app: &App) -> Element<'_, Message> {
    ManageMenu::ALL
        .into_iter()
        .fold(column![].spacing(2), |column, item| {
            let progress = app.sidebar_menu_progress[item.index()];
            column.push(sidebar_item(
                manage_menu_icon(
                    item,
                    mix_color(Color::from_rgb8(148, 163, 184), color_focus(), progress),
                )
                .into(),
                item.title(),
                progress,
                app.selected_menu == item,
                Message::SelectMenu(item),
            ))
        })
        .into()
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
            } else {
                match tab.connection_type {
                    ConnectionType::Local => "L",
                    ConnectionType::Serial => "R",
                    ConnectionType::Ssh => "T",
                }
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
                embedded_horizontal_scrollbar()
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
    .width(Length::Fill)
}

fn labeled_pick_list_owned<'a>(
    label: &'a str,
    options: Vec<String>,
    selected: Option<String>,
    placeholder: &'a str,
    on_select: impl Fn(String) -> Message + 'static,
) -> iced::widget::Column<'a, Message> {
    column![
        text(label)
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(color_text_muted()),
        pick_list(options, selected, on_select)
            .placeholder(placeholder)
            .width(Length::Fill)
            .menu_height(Length::Fixed(PICK_LIST_MENU_MAX_HEIGHT))
            .padding([10, 12])
            .style(pick_list_field_style)
            .menu_style(pick_list_menu_style),
    ]
    .spacing(6)
    .width(Length::Fill)
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
