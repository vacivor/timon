use super::*;

pub(crate) fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::MainWindowReady(id) => {
            app.main_window = Some(id);
            app.resize_terminals();
            if !app.main_window_vibrancy_installed {
                app.main_window_vibrancy_installed = true;
                return install_main_window_vibrancy(id);
            }
        }
        Message::SettingsWindowReady(id) => {
            app.settings_window = Some(id);
        }
        Message::Tick => {
            let mut log_messages = Vec::new();

            for tab in &mut app.terminal_tabs {
                while let Some(event) = tab.terminal.try_recv_event() {
                    match event {
                        TerminalEvent::Title(title) => {
                            let trimmed = title.trim();
                            if !trimmed.is_empty() {
                                tab.title = trimmed.to_string();
                            }
                        }
                        TerminalEvent::ResetTitle => {
                            tab.title = tab.fallback_title.clone();
                        }
                    }
                }

                let mut pending = Vec::new();
                if let Some(rx) = &mut tab.event_rx {
                    while let Ok(event) = rx.try_recv() {
                        pending.push(event);
                    }
                }

                for event in pending {
                    match event {
                        SessionEvent::Connected { description } => {
                            tab.status = description.clone();
                            log_messages.push(format!("{}: {}", tab.title, description));
                        }
                        SessionEvent::Output(bytes) => tab.terminal.feed(&bytes),
                        SessionEvent::Status(status) => {
                            tab.status = status.clone();
                            log_messages.push(format!("{}: {}", tab.title, status));
                        }
                        SessionEvent::Error(error) => {
                            tab.status = error.clone();
                            tab.terminal.push_local_line(&format!("Error: {error}"));
                            log_messages.push(format!("{}: {error}", tab.title));
                        }
                        SessionEvent::Disconnected(reason) => {
                            tab.status = reason.clone();
                            tab.terminal
                                .push_local_line(&format!("Disconnected: {reason}"));
                            tab.session = None;
                            log_messages.push(format!("{}: {reason}", tab.title));
                        }
                    }
                }
            }

            for log in log_messages {
                app.log(log);
            }

            app.animate_titlebar_tabs();
        }
        Message::WindowEvent(id, event) => match event {
            window::Event::Opened { size, .. } => {
                if Some(id) == app.main_window {
                    app.main_window_size = size;
                    app.resize_terminals();
                    if !app.main_window_vibrancy_installed {
                        app.main_window_vibrancy_installed = true;
                        return install_main_window_vibrancy(id);
                    }
                }
            }
            window::Event::Resized(size) => {
                if Some(id) == app.main_window {
                    app.main_window_size = size;
                    app.resize_terminals();
                }
            }
            window::Event::Rescaled(scale_factor) => {
                if Some(id) == app.main_window {
                    app.main_window_scale_factor = scale_factor.max(1.0);
                    app.glyph_atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
                }
            }
            window::Event::Closed => {
                if Some(id) == app.settings_window {
                    app.settings_window = None;
                }
                if Some(id) == app.main_window {
                    app.main_window_vibrancy_installed = false;
                    return iced::exit();
                }
            }
            _ => {}
        },
        Message::KeyboardInput(
            id,
            keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                text,
                ..
            },
        ) => {
            if is_open_settings_shortcut(&app.settings.shortcuts, &key, physical_key, modifiers) {
                return open_settings_window(app);
            }

            if is_minimize_window_shortcut(&app.settings.shortcuts, &key, physical_key, modifiers) {
                return window::minimize(id, true);
            }

            if Some(id) == app.settings_window {
                if is_close_tab_shortcut(&app.settings.shortcuts, &key, physical_key, modifiers) {
                    return window::close(id);
                }

                return Task::none();
            }

            if Some(id) == app.main_window {
                if is_close_tab_shortcut(&app.settings.shortcuts, &key, physical_key, modifiers) {
                    if !app.close_active_terminal_tab() && app.active_tab == ActiveTab::Manage {
                        return window::close(id);
                    }
                    return Task::none();
                }

                if let Some(index) = tab_switch_shortcut_index(
                    &app.settings.shortcuts,
                    &key,
                    physical_key,
                    modifiers,
                ) {
                    app.activate_tab_index(index);
                    return Task::none();
                }

                if is_previous_tab_shortcut(&app.settings.shortcuts, &key, physical_key, modifiers)
                {
                    app.activate_adjacent_tab(-1);
                    return Task::none();
                }

                if is_next_tab_shortcut(&app.settings.shortcuts, &key, physical_key, modifiers) {
                    app.activate_adjacent_tab(1);
                    return Task::none();
                }

                if is_copy_shortcut(&key, modifiers) {
                    if let Some(focused) = app.terminal_composer_focus {
                        if app.active_tab == ActiveTab::Terminal(focused) {
                            return Task::none();
                        }
                    }

                    if let Some(tab_id) = app.terminal_focus {
                        if let Some(tab) = app.terminal_tabs.iter().find(|tab| tab.id == tab_id) {
                            let snapshot =
                                tab.terminal.snapshot(&app.terminal_theme(&tab.theme_id));
                            if let Some(contents) =
                                selection_contents(&snapshot, tab.selection.as_ref())
                            {
                                return iced::clipboard::write(contents);
                            }
                        }
                    }
                    return Task::none();
                }

                if let Some(tab_id) = app.terminal_focus {
                    if app.terminal_composer_focus == Some(tab_id) {
                        return Task::none();
                    }

                    if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                        if let Some(bytes) = tab.terminal.encode_key(
                            key,
                            modifiers,
                            text.map(|value| value.to_string()),
                        ) {
                            if let Some(session) = &tab.session {
                                let _ = session.command_tx.send(SessionCommand::Input(bytes));
                            }
                        }
                    }
                }
            }
        }
        Message::InputMethod(id, input_method::Event::Commit(content)) => {
            if Some(id) == app.main_window {
                if let Some(tab_id) = app.terminal_focus {
                    if app.terminal_composer_focus == Some(tab_id) {
                        return Task::none();
                    }

                    if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                        if let Some(session) = &tab.session {
                            let _ = session
                                .command_tx
                                .send(SessionCommand::Input(content.into_bytes()));
                        }
                    }
                }
            }
        }
        Message::InputMethod(_, _) => {}
        Message::KeyboardInput(_, _) => {}
        Message::DragWindow(id) => return window::drag(id),
        Message::SelectMenu(menu) => {
            app.selected_menu = menu;
            app.active_profile_context = None;
            app.active_key_context = None;
            app.active_identity_context = None;

            if menu == ManageMenu::Settings {
                return open_settings_window(app);
            }
        }
        Message::OpenSettingsWindow => return open_settings_window(app),
        Message::ActivateManageTab => {
            app.active_tab = ActiveTab::Manage;
            app.terminal_focus = None;
            app.terminal_composer_focus = None;
            app.resize_terminals();
        }
        Message::ActivateTerminal(id) => {
            app.active_tab = ActiveTab::Terminal(id);
            app.terminal_focus = Some(id);
            app.terminal_composer_focus = None;
            app.resize_terminals();
        }
        Message::CloseTerminal(id) => app.close_terminal_tab(id),
        Message::TerminalSelectionStarted(id, point) => {
            app.active_tab = ActiveTab::Terminal(id);
            app.terminal_focus = Some(id);
            app.terminal_composer_focus = None;

            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                tab.selection_anchor = Some(point);
                tab.selection = Some(TerminalSelection {
                    start: point,
                    end: point,
                });
            }
        }
        Message::TerminalSelectionUpdated(id, point) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                let anchor = tab.selection_anchor.unwrap_or(point);
                tab.selection_anchor = Some(anchor);
                tab.selection = Some(normalize_selection(anchor, point));
            }
        }
        Message::TerminalScrolled(id, lines, point) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                tab.terminal.handle_scroll(lines, point);
            }
        }
        Message::TerminalComposerAction(id, action) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                app.terminal_composer_focus = Some(id);
                tab.composer.perform(action);
                if app.active_tab == ActiveTab::Terminal(id) {
                    app.resize_terminals();
                }
            }
        }
        Message::SubmitTerminalComposer(id) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                let command = tab.composer.text();
                let payload = command.trim_end_matches('\n').trim_end_matches('\r');

                if !payload.trim().is_empty() {
                    if let Some(session) = &tab.session {
                        let mut bytes = payload.as_bytes().to_vec();
                        bytes.push(b'\n');
                        let _ = session.command_tx.send(SessionCommand::Input(bytes));
                    }
                }

                tab.composer = text_editor::Content::new();
                app.terminal_composer_focus = None;
                if app.active_tab == ActiveTab::Terminal(id) {
                    app.resize_terminals();
                }
            }

            return iced::advanced::widget::operate(
                iced::advanced::widget::operation::focusable::unfocus(),
            );
        }
        Message::OpenProfileContext(id) => {
            app.active_profile_context = Some(id);
            app.active_key_context = None;
            app.active_identity_context = None;
        }
        Message::OpenKeyContext(id) => {
            app.active_profile_context = None;
            app.active_key_context = Some(id);
            app.active_identity_context = None;
        }
        Message::OpenIdentityContext(id) => {
            app.active_profile_context = None;
            app.active_key_context = None;
            app.active_identity_context = Some(id);
        }
        Message::DuplicateProfile(id) => {
            if let Some(original) = app
                .profiles
                .iter()
                .find(|profile| profile.id == id)
                .cloned()
            {
                let mut duplicated = original;
                duplicated.id = 0;
                duplicated.name = if duplicated.name.trim().is_empty() {
                    "Profile Copy".into()
                } else {
                    format!("{} Copy", duplicated.name)
                };

                match app.database.save_profile(&mut duplicated) {
                    Ok(()) => {
                        app.reload_data();
                        app.active_profile_context = None;
                        app.log(format!("Duplicated profile: {}", duplicated.name));
                    }
                    Err(error) => {
                        app.log(format!("Failed to duplicate profile: {error}"));
                    }
                }
            }
        }
        Message::NewProfile => {
            app.drawer = Some(DrawerState::Profile(ProfileEditor::new()));
            app.active_profile_context = None;
        }
        Message::EditProfile(id) => {
            if let Some(profile) = app.profiles.iter().find(|profile| profile.id == id) {
                app.drawer = Some(DrawerState::Profile(ProfileEditor::from_profile(profile)));
                app.active_profile_context = None;
            }
        }
        Message::ProfileFieldChanged(field, value) => {
            if let Some(DrawerState::Profile(editor)) = &mut app.drawer {
                match field {
                    ProfileField::Name => editor.name = value,
                    ProfileField::GroupId => editor.group_id = value,
                    ProfileField::KeyId => editor.key_id = value,
                    ProfileField::IdentityId => editor.identity_id = value,
                    ProfileField::Host => editor.host = value,
                    ProfileField::Port => editor.port = value,
                    ProfileField::Username => editor.username = value,
                    ProfileField::Password => editor.password = value,
                    ProfileField::ThemeId => editor.theme_id = value,
                    ProfileField::ShellPath => {
                        editor.shell_path = if value == "Login Shell" {
                            String::new()
                        } else {
                            value
                        };
                    }
                    ProfileField::WorkDir => editor.work_dir = value,
                    ProfileField::StartupCommand => editor.startup_command = value,
                }
            }
        }
        Message::ProfileTypeChanged(value) => {
            if let Some(DrawerState::Profile(editor)) = &mut app.drawer {
                editor.profile_type = ProfileType::from(value.as_str());
                if editor.profile_type != ProfileType::Local {
                    editor.shell_path.clear();
                    editor.work_dir.clear();
                }
            }
        }
        Message::SaveProfile => {
            if let Some(DrawerState::Profile(editor)) = &app.drawer {
                match editor.to_profile() {
                    Ok(mut profile) => match app.database.save_profile(&mut profile) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 profile: {}", profile.name));
                        }
                        Err(error) => app.log(format!("保存 profile 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::NewKey => {
            app.drawer = Some(DrawerState::Key(KeyEditor::new()));
            app.active_key_context = None;
        }
        Message::EditKey(id) => {
            if let Some(key) = app.keys.iter().find(|key| key.id == id) {
                app.drawer = Some(DrawerState::Key(KeyEditor::from_key(key)));
                app.active_key_context = None;
            }
        }
        Message::KeyFieldChanged(field, value) => {
            if let Some(DrawerState::Key(editor)) = &mut app.drawer {
                match field {
                    KeyField::Name => editor.name = value,
                }
            }
        }
        Message::KeyEditorAction(field, action) => {
            if let Some(DrawerState::Key(editor)) = &mut app.drawer {
                match field {
                    KeyTextField::PrivateKey => {
                        editor.private_key_content.perform(action);
                        editor.private_key = editor.private_key_content.text();
                    }
                    KeyTextField::PublicKey => {
                        editor.public_key_content.perform(action);
                        editor.public_key = editor.public_key_content.text();
                    }
                    KeyTextField::Certificate => {
                        editor.certificate_content.perform(action);
                        editor.certificate = editor.certificate_content.text();
                    }
                }
            }
        }
        Message::SaveKey => {
            if let Some(DrawerState::Key(editor)) = &app.drawer {
                match editor.to_key() {
                    Ok(mut key) => match app.database.save_key(&mut key) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 key: {}", key.name));
                        }
                        Err(error) => app.log(format!("保存 key 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::NewIdentity => {
            app.drawer = Some(DrawerState::Identity(IdentityEditor::new()));
            app.active_identity_context = None;
        }
        Message::EditIdentity(id) => {
            if let Some(identity) = app.identities.iter().find(|identity| identity.id == id) {
                app.drawer = Some(DrawerState::Identity(IdentityEditor::from_identity(
                    identity,
                )));
                app.active_identity_context = None;
            }
        }
        Message::IdentityFieldChanged(field, value) => {
            if let Some(DrawerState::Identity(editor)) = &mut app.drawer {
                match field {
                    IdentityField::Name => editor.name = value,
                    IdentityField::Username => editor.username = value,
                    IdentityField::Password => editor.password = value,
                    IdentityField::KeyId => editor.key_id = value,
                }
            }
        }
        Message::SaveIdentity => {
            if let Some(DrawerState::Identity(editor)) = &app.drawer {
                match editor.to_identity() {
                    Ok(mut identity) => match app.database.save_identity(&mut identity) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 identity: {}", identity.name));
                        }
                        Err(error) => app.log(format!("保存 identity 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::CloseDrawer => app.drawer = None,
        Message::ConnectProfile(id) => return app.open_terminal_from_profile(id),
        Message::TerminalConnected(id, result) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                match result {
                    Ok(handle) => {
                        tab.terminal.set_outbound(handle.command_tx.clone());
                        tab.session = Some(handle);
                        tab.status = "Connected".into();
                    }
                    Err(error) => {
                        tab.status = error.clone();
                        tab.terminal
                            .push_local_line(&format!("Connection error: {error}"));
                        let title = tab.title.clone();
                        app.log(format!("{title}: {error}"));
                    }
                }
            }
        }
        Message::SettingsFontChanged(field, value) => match field {
            FontField::Family => app.settings_editor.font_family = value,
            FontField::Size => app.settings_editor.font_size = value,
            FontField::LineHeight => app.settings_editor.line_height = value,
        },
        Message::SettingsScrollbackChanged(value) => {
            app.settings_editor.scrollback_lines = value;
        }
        Message::SettingsFontThickenChanged(value) => {
            app.settings_editor.font_thicken = value;
        }
        Message::SettingsCursorChanged(CursorField::Shape, value) => {
            app.settings_editor.cursor_shape = value;
        }
        Message::SettingsCursorBlinkChanged(value) => {
            app.settings_editor.cursor_blinking = value;
        }
        Message::SettingsColorChanged(field, value) => match field {
            ColorField::Background => app.settings_editor.background = value,
            ColorField::Foreground => app.settings_editor.foreground = value,
            ColorField::CursorColor => app.settings_editor.cursor_color = value,
            ColorField::CursorText => app.settings_editor.cursor_text = value,
            ColorField::SelectionBackground => app.settings_editor.selection_background = value,
            ColorField::SelectionForeground => app.settings_editor.selection_foreground = value,
            ColorField::AnsiNormal(index) => {
                if let Some(slot) = app.settings_editor.ansi_normal.get_mut(index) {
                    *slot = value;
                }
            }
            ColorField::AnsiBright(index) => {
                if let Some(slot) = app.settings_editor.ansi_bright.get_mut(index) {
                    *slot = value;
                }
            }
        },
        Message::SaveSettings => {
            app.settings_editor.font_family = normalize_font_family_choice(
                &app.settings_editor.font_family,
                &app.available_fonts,
            );
            app.settings_editor.apply_to_settings(&mut app.settings);
            match save_settings(&app.paths.settings, &app.settings) {
                Ok(()) => {
                    app.terminal_font = TerminalFont::from_settings(&app.settings.terminal.font);
                    app.glyph_atlas = Arc::new(Mutex::new(GlyphAtlas::new()));
                    let terminal_settings = app.settings.terminal.clone();
                    for tab in &mut app.terminal_tabs {
                        tab.terminal.reset(&terminal_settings);
                    }
                    app.log(format!("设置已保存到 {}", app.paths.settings.display()));
                    app.resize_terminals();
                }
                Err(error) => app.log(format!("保存设置失败: {error:#}")),
            }
        }
        Message::ResetThemeToAtomOneLight => {
            let colors = TerminalColors::atom_one_light();
            app.settings_editor.background = colors.background;
            app.settings_editor.foreground = colors.foreground;
            app.settings_editor.cursor_color = colors.cursor_color;
            app.settings_editor.cursor_text = colors.cursor_text;
            app.settings_editor.selection_background = colors.selection_background;
            app.settings_editor.selection_foreground = colors.selection_foreground;
            app.settings_editor.ansi_normal = colors.normal.as_array();
            app.settings_editor.ansi_bright = colors.bright.as_array();
        }
    }

    Task::none()
}

pub(crate) fn open_settings_window(app: &mut App) -> Task<Message> {
    if app.settings_window.is_some() {
        return Task::none();
    }

    let (_id, task) = window::open(settings_window_settings());
    task.map(Message::SettingsWindowReady)
}
