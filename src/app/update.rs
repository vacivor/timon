use super::*;

pub(crate) fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::MainWindowReady(id) => {
            app.main_window = Some(id);
            app.resize_terminals();
        }
        Message::CursorMoved(position) => {
            app.cursor_position = Some(position);
        }
        Message::SettingsWindowReady(id) => {
            app.settings_window = Some(id);
        }
        Message::Tick => {
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
            }

            app.animate_titlebar_tabs();
            app.animate_sidebar_menu();
        }
        Message::WindowEvent(id, event) => match event {
            window::Event::Opened { size, .. } => {
                if Some(id) == app.main_window {
                    app.main_window_size = size;
                    app.resize_terminals();
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
                    app.prewarm_terminal_glyphs();
                }
            }
            window::Event::Closed => {
                if Some(id) == app.settings_window {
                    app.settings_window = None;
                }
                if Some(id) == app.main_window {
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
            app.keyboard_modifiers = modifiers;

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

                    if let Some(focused) = app.terminal_composer_focus {
                        if app.active_tab == ActiveTab::Terminal(focused) {
                            return Task::none();
                        }
                    }

                    return Task::none();
                }

                if is_paste_shortcut(&key, modifiers) {
                    if let Some(tab_id) = app.terminal_focus {
                        if app.terminal_composer_focus == Some(tab_id) {
                            return Task::none();
                        }

                        return iced::clipboard::read()
                            .map(move |content| Message::TerminalPaste(tab_id, content));
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
                            tab.terminal.scroll_to_bottom();
                            if let Some(session) = &tab.session {
                                let _ = session.command_tx.send(SessionCommand::Input(bytes));
                            }
                        }
                    }
                }
            }
        }
        Message::KeyboardInput(_, keyboard::Event::KeyReleased { modifiers, .. }) => {
            app.keyboard_modifiers = modifiers;
        }
        Message::KeyboardInput(_, keyboard::Event::ModifiersChanged(modifiers)) => {
            app.keyboard_modifiers = modifiers;
        }
        Message::InputMethod(id, input_method::Event::Commit(content)) => {
            if Some(id) == app.main_window {
                if let Some(tab_id) = app.terminal_focus {
                    if app.terminal_composer_focus == Some(tab_id) {
                        return Task::none();
                    }

                    if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                        if let Some(session) = &tab.session {
                            let bytes = if content.contains('\n') || content.contains('\r') {
                                tab.terminal.encode_text_input(&content)
                            } else {
                                content.into_bytes()
                            };
                            tab.terminal.scroll_to_bottom();
                            let _ = session.command_tx.send(SessionCommand::Input(bytes));
                        }
                    }
                }
            }
        }
        Message::InputMethod(_, _) => {}
        Message::DragWindow(id) => return window::drag(id),
        Message::SelectMenu(menu) => {
            app.selected_menu = menu;
            let progress = &mut app.sidebar_menu_progress[menu.index()];
            if *progress < SIDEBAR_MENU_ACTIVATE_PRIME {
                *progress = SIDEBAR_MENU_ACTIVATE_PRIME;
            }
            app.context_menu = None;
            app.active_group_context = None;
            app.active_port_forward_context = None;

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
            app.context_menu = None;
        }
        Message::ActivateTerminal(id) => {
            app.active_tab = ActiveTab::Terminal(id);
            app.terminal_focus = Some(id);
            app.terminal_composer_focus = None;
            app.resize_terminals();
            app.context_menu = None;
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
        Message::TerminalSelectionWord(id, selection) => {
            app.active_tab = ActiveTab::Terminal(id);
            app.terminal_focus = Some(id);
            app.terminal_composer_focus = None;

            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                tab.selection_anchor = Some(selection.start);
                tab.selection = Some(selection);
            }
        }
        Message::TerminalSelectionToken(id, selection) => {
            app.active_tab = ActiveTab::Terminal(id);
            app.terminal_focus = Some(id);
            app.terminal_composer_focus = None;

            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                tab.selection_anchor = Some(selection.start);
                tab.selection = Some(selection);
            }
        }
        Message::TerminalCommandClick(id, point) => {
            if let Some(tab) = app.terminal_tabs.iter().find(|tab| tab.id == id) {
                let theme = app.terminal_theme(&tab.theme_id);
                let snapshot = tab.terminal.snapshot(&theme);
                let Some(selection) = tab.terminal.clickable_selection_at_point(&theme, point)
                else {
                    return Task::none();
                };
                let Some(token) = selection_contents(&snapshot, Some(&selection))
                    .map(|value| normalize_clickable_token(&value))
                    .filter(|value| !value.is_empty())
                else {
                    return Task::none();
                };

                match tab.connection_type {
                    ConnectionType::Local => {
                        if let Some(target) = local_open_target_with_base(&token, &tab.work_dir) {
                            if let Err(error) = open_external_target(&target) {
                                app.log(format!("打开目标失败 {target}: {error}"));
                            }
                        }
                    }
                    ConnectionType::Ssh | ConnectionType::Serial => {
                        if is_supported_remote_url(&token) {
                            if let Err(error) = open_external_target(&token) {
                                app.log(format!("打开网址失败 {token}: {error}"));
                            }
                        }
                    }
                }
            }
        }
        Message::TerminalScrolled(id, lines, point) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                tab.terminal.handle_scroll(lines, point);
            }
        }
        Message::TerminalResized(id, cols, rows) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                let cols = cols.max(2);
                let rows = rows.max(2);
                if tab.terminal.dimensions() != (cols, rows) {
                    tab.terminal.resize(cols, rows);
                    if let Some(session) = &tab.session {
                        let _ = session.command_tx.send(SessionCommand::Resize {
                            cols: cols as u16,
                            rows: rows as u16,
                        });
                    }
                }
            }
        }
        Message::TerminalPaste(id, Some(content)) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                if let Some(session) = &tab.session {
                    tab.terminal.scroll_to_bottom();
                    let _ = session.command_tx.send(SessionCommand::Input(
                        tab.terminal.encode_text_input(&content),
                    ));
                }
            }
        }
        Message::TerminalPaste(_, None) => {}
        Message::TerminalComposerAction(id, action) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                app.terminal_composer_focus = Some(id);
                if action.is_edit() {
                    tab.reset_composer_history_navigation();
                }
                tab.composer.perform(action);
                if app.active_tab == ActiveTab::Terminal(id) {
                    app.resize_terminals();
                }
            }
        }
        Message::TerminalComposerHistoryPrev(id) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                app.terminal_composer_focus = Some(id);
                if tab.composer_history_prev() && app.active_tab == ActiveTab::Terminal(id) {
                    app.resize_terminals();
                }
            }
        }
        Message::TerminalComposerHistoryNext(id) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                app.terminal_composer_focus = Some(id);
                if tab.composer_history_next() && app.active_tab == ActiveTab::Terminal(id) {
                    app.resize_terminals();
                }
            }
        }
        Message::SubmitTerminalComposer(id) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                let command = tab.composer.text();
                let payload = command.trim_end_matches('\n').trim_end_matches('\r');

                if !payload.trim().is_empty() {
                    tab.push_composer_history(payload);
                    if let Some(session) = &tab.session {
                        let mut bytes = tab.terminal.encode_text_input(payload);
                        bytes.push(b'\r');
                        tab.terminal.scroll_to_bottom();
                        let _ = session.command_tx.send(SessionCommand::Input(bytes));
                    }
                } else {
                    tab.reset_composer_history_navigation();
                }

                tab.set_composer_text("");
                app.terminal_composer_focus = Some(id);
                if app.active_tab == ActiveTab::Terminal(id) {
                    app.resize_terminals();
                }
            }
        }
        Message::OpenConnectionContext(id) => {
            app.context_menu = Some(ContextMenuState {
                target: ContextMenuTarget::Connection(id),
                position: app.cursor_position,
            });
            app.active_group_context = None;
            app.active_port_forward_context = None;
        }
        Message::OpenKeyContext(id) => {
            app.context_menu = Some(ContextMenuState {
                target: ContextMenuTarget::Key(id),
                position: app.cursor_position,
            });
            app.active_group_context = None;
            app.active_port_forward_context = None;
        }
        Message::OpenIdentityContext(id) => {
            app.context_menu = Some(ContextMenuState {
                target: ContextMenuTarget::Identity(id),
                position: app.cursor_position,
            });
            app.active_group_context = None;
            app.active_port_forward_context = None;
        }
        Message::DuplicateConnection(id) => {
            if let Some(original) = app
                .connections
                .iter()
                .find(|connection| connection.id == id)
                .cloned()
            {
                let mut duplicated = original;
                duplicated.id = 0;
                duplicated.name = if duplicated.name.trim().is_empty() {
                    "Connection Copy".into()
                } else {
                    format!("{} Copy", duplicated.name)
                };

                match app.database.save_connection(&mut duplicated) {
                    Ok(()) => {
                        app.reload_data();
                        app.context_menu = None;
                        app.log(format!("Duplicated connection: {}", duplicated.name));
                    }
                    Err(error) => {
                        app.log(format!("Failed to duplicate connection: {error}"));
                    }
                }
            }
        }
        Message::DeleteConnection(id) => {
            let deleted_name = app
                .connections
                .iter()
                .find(|connection| connection.id == id)
                .map(|connection| connection.name.clone())
                .unwrap_or_else(|| format!("Connection #{id}"));

            match app.database.delete_connection(id) {
                Ok(()) => {
                    app.reload_data();
                    app.context_menu = None;
                    app.log(format!("Deleted connection: {deleted_name}"));
                }
                Err(error) => {
                    app.log(format!("Failed to delete connection: {error}"));
                }
            }
        }
        Message::NewConnection => {
            app.drawer = Some(DrawerState::Connection(ConnectionEditor::new()));
            app.context_menu = None;
        }
        Message::NewSerialConnection => {
            let mut editor = ConnectionEditor::new();
            editor.name = "Serial".into();
            editor.connection_type = ConnectionType::Serial;
            editor.baud_rate = "115200".into();
            app.drawer = Some(DrawerState::Connection(editor));
            app.context_menu = None;
        }
        Message::EditConnection(id) => {
            if let Some(connection) = app
                .connections
                .iter()
                .find(|connection| connection.id == id)
            {
                app.drawer = Some(DrawerState::Connection(ConnectionEditor::from_connection(
                    connection,
                )));
                app.context_menu = None;
            }
        }
        Message::OpenSftpConnection(id) => return app.open_sftp_from_connection(id),
        Message::ConnectionFieldChanged(field, value) => {
            if let Some(DrawerState::Connection(editor)) = &mut app.drawer {
                match field {
                    ConnectionField::Name => editor.name = value,
                    ConnectionField::GroupId => editor.group_id = value,
                    ConnectionField::KeyId => editor.key_id = value,
                    ConnectionField::IdentityId => editor.identity_id = value,
                    ConnectionField::Host => editor.host = value,
                    ConnectionField::Port => editor.port = value,
                    ConnectionField::Username => editor.username = value,
                    ConnectionField::Password => editor.password = value,
                    ConnectionField::ThemeId => editor.theme_id = value,
                    ConnectionField::ShellPath => {
                        editor.shell_path = if value == "Login Shell" {
                            String::new()
                        } else {
                            value
                        };
                    }
                    ConnectionField::WorkDir => editor.work_dir = value,
                    ConnectionField::StartupCommand => editor.startup_command = value,
                    ConnectionField::SerialPort => editor.serial_port = value,
                    ConnectionField::BaudRate => editor.baud_rate = value,
                }
            }
        }
        Message::ConnectionTypeChanged(value) => {
            if let Some(DrawerState::Connection(editor)) = &mut app.drawer {
                editor.connection_type = ConnectionType::from(value.as_str());
                match editor.connection_type {
                    ConnectionType::Local => {
                        editor.serial_port.clear();
                        editor.baud_rate = "115200".into();
                        if editor.port.trim().is_empty() {
                            editor.port = "0".into();
                        }
                    }
                    ConnectionType::Ssh => {
                        editor.shell_path.clear();
                        editor.work_dir.clear();
                        editor.serial_port.clear();
                        editor.baud_rate = "115200".into();
                        if editor.port.trim() == "0" {
                            editor.port = "22".into();
                        }
                    }
                    ConnectionType::Serial => {
                        editor.shell_path.clear();
                        editor.work_dir.clear();
                        editor.host.clear();
                        editor.port = "0".into();
                        editor.key_id = "None".into();
                        editor.identity_id = "None".into();
                        editor.username.clear();
                        editor.password.clear();
                        if editor.baud_rate.trim().is_empty() || editor.baud_rate.trim() == "0" {
                            editor.baud_rate = "115200".into();
                        }
                    }
                }
            }
        }
        Message::SaveConnection => {
            if let Some(DrawerState::Connection(editor)) = &app.drawer {
                match editor.to_connection() {
                    Ok(mut connection) => match app.database.save_connection(&mut connection) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 connection: {}", connection.name));
                        }
                        Err(error) => app.log(format!("保存 connection 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::NewGroup => {
            app.drawer = Some(DrawerState::Group(GroupEditor::new()));
            app.active_group_context = None;
        }
        Message::EditGroup(id) => {
            if let Some(group) = app.groups.iter().find(|group| group.id == id) {
                app.drawer = Some(DrawerState::Group(GroupEditor::from_group(group)));
                app.active_group_context = None;
            }
        }
        Message::GroupFieldChanged(field, value) => {
            if let Some(DrawerState::Group(editor)) = &mut app.drawer {
                match field {
                    GroupField::Name => editor.name = value,
                    GroupField::ParentId => editor.parent_id = value,
                }
            }
        }
        Message::SaveGroup => {
            if let Some(DrawerState::Group(editor)) = &app.drawer {
                match editor.to_group() {
                    Ok(mut group) => match app.database.save_group(&mut group) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存 group: {}", group.name));
                        }
                        Err(error) => app.log(format!("保存 group 失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::NewKey => {
            app.drawer = Some(DrawerState::Key(KeyEditor::new()));
            app.context_menu = None;
        }
        Message::EditKey(id) => {
            if let Some(key) = app.keys.iter().find(|key| key.id == id) {
                app.drawer = Some(DrawerState::Key(KeyEditor::from_key(key)));
                app.context_menu = None;
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
            app.context_menu = None;
        }
        Message::EditIdentity(id) => {
            if let Some(identity) = app.identities.iter().find(|identity| identity.id == id) {
                app.drawer = Some(DrawerState::Identity(IdentityEditor::from_identity(
                    identity,
                )));
                app.context_menu = None;
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
        Message::NewPortForward => {
            app.drawer = Some(DrawerState::PortForward(PortForwardEditor::new()));
            app.active_port_forward_context = None;
        }
        Message::EditPortForward(id) => {
            if let Some(forward) = app.port_forwards.iter().find(|forward| forward.id == id) {
                app.drawer = Some(DrawerState::PortForward(
                    PortForwardEditor::from_port_forward(forward),
                ));
                app.active_port_forward_context = None;
            }
        }
        Message::PortForwardFieldChanged(field, value) => {
            if let Some(DrawerState::PortForward(editor)) = &mut app.drawer {
                match field {
                    PortForwardField::Label => editor.label = value,
                    PortForwardField::BindAddress => editor.bind_address = value,
                    PortForwardField::BindPort => editor.bind_port = value,
                    PortForwardField::ConnectionId => editor.connection_id = value,
                    PortForwardField::DestinationHost => editor.destination_host = value,
                    PortForwardField::DestinationPort => editor.destination_port = value,
                }
            }
        }
        Message::PortForwardTypeChanged(value) => {
            if let Some(DrawerState::PortForward(editor)) = &mut app.drawer {
                editor.forward_type = PortForwardType::from(value.as_str());
            }
        }
        Message::SavePortForward => {
            if let Some(DrawerState::PortForward(editor)) = &app.drawer {
                match editor.to_port_forward() {
                    Ok(mut forward) => match app.database.save_port_forward(&mut forward) {
                        Ok(()) => {
                            app.reload_data();
                            app.drawer = None;
                            app.log(format!("已保存端口转发: {}", forward.label));
                        }
                        Err(error) => app.log(format!("保存端口转发失败: {error:#}")),
                    },
                    Err(error) => app.log(error),
                }
            }
        }
        Message::DeletePortForward(id) => {
            if let Some(runtime) = app.port_forward_runtimes.remove(&id) {
                runtime.stop();
            }
            match app.database.delete_port_forward(id) {
                Ok(()) => {
                    app.reload_data();
                    app.active_port_forward_context = None;
                }
                Err(error) => app.log(format!("删除端口转发失败: {error:#}")),
            }
        }
        Message::TogglePortForward(id, enabled) => {
            let Some(forward) = app
                .port_forwards
                .iter()
                .find(|forward| forward.id == id)
                .cloned()
            else {
                return Task::none();
            };

            if !enabled {
                if let Some(runtime) = app.port_forward_runtimes.remove(&id) {
                    runtime.stop();
                }
                if let Some(stored) = app.port_forwards.iter_mut().find(|item| item.id == id) {
                    stored.enabled = false;
                }
                let mut updated = forward;
                updated.enabled = false;
                if let Err(error) = app.database.save_port_forward(&mut updated) {
                    app.log(format!("更新端口转发状态失败: {error:#}"));
                }
                app.reload_data();
                return Task::none();
            }

            let Some(connection_id) = forward.connection_id else {
                app.log("端口转发需要绑定一个 SSH connection");
                return Task::none();
            };
            let Some(connection) = app
                .connections
                .iter()
                .find(|connection| connection.id == connection_id)
                .cloned()
            else {
                app.log("端口转发绑定的 connection 不存在");
                return Task::none();
            };
            let key = connection
                .effective_key_id
                .and_then(|id| app.keys.iter().find(|key| key.id == id))
                .cloned();
            let identity = connection
                .identity_id
                .and_then(|id| app.identities.iter().find(|identity| identity.id == id))
                .cloned();
            let target = ConnectionTarget {
                connection,
                key,
                identity,
                known_hosts_path: app.paths.known_hosts.clone(),
                cols: 80,
                rows: 24,
            };
            return Task::perform(start_port_forward(target, forward), move |result| {
                Message::PortForwardStarted(id, result)
            });
        }
        Message::PortForwardStarted(id, result) => match result {
            Ok(handle) => {
                app.port_forward_runtimes.insert(id, handle);
                if let Some(forward) = app
                    .port_forwards
                    .iter_mut()
                    .find(|forward| forward.id == id)
                {
                    forward.enabled = true;
                    let mut updated = forward.clone();
                    let _ = app.database.save_port_forward(&mut updated);
                }
                app.reload_data();
            }
            Err(error) => {
                app.log(format!("端口转发启动失败: {error}"));
                if let Some(forward) = app
                    .port_forwards
                    .iter_mut()
                    .find(|forward| forward.id == id)
                {
                    forward.enabled = false;
                    let mut updated = forward.clone();
                    let _ = app.database.save_port_forward(&mut updated);
                }
                app.reload_data();
            }
        },
        Message::SftpConnected(id, result) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                if let TabWorkspace::Sftp(sftp) = &mut tab.workspace {
                    match result {
                        Ok(handle) => {
                            sftp.handle = Some(handle.clone());
                            sftp.loading = true;
                            tab.status = "Connected".into();
                            let current_path = sftp.current_path.clone();
                            return Task::perform(
                                sftp_list_dir(handle, current_path.clone()),
                                move |result| {
                                    Message::SftpDirectoryLoaded(id, current_path, result)
                                },
                            );
                        }
                        Err(error) => {
                            sftp.loading = false;
                            tab.status = error.clone();
                            app.log(error);
                        }
                    }
                }
            }
        }
        Message::SftpDirectoryLoaded(id, path, result) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                if let TabWorkspace::Sftp(sftp) = &mut tab.workspace {
                    sftp.loading = false;
                    match result {
                        Ok(entries) => {
                            sftp.current_path = path.clone();
                            sftp.entries = entries;
                            sftp.selected_file = None;
                            sftp.preview.clear();
                            tab.title = format!("{} · {}", tab.fallback_title, path);
                        }
                        Err(error) => {
                            tab.status = error.clone();
                            app.log(error);
                        }
                    }
                }
            }
        }
        Message::SftpFileLoaded(id, path, result) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                if let TabWorkspace::Sftp(sftp) = &mut tab.workspace {
                    sftp.loading = false;
                    match result {
                        Ok(preview) => {
                            sftp.selected_file = Some(path);
                            sftp.preview = preview;
                        }
                        Err(error) => {
                            tab.status = error.clone();
                            app.log(error);
                        }
                    }
                }
            }
        }
        Message::SftpNavigate(id, path) => {
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                if let TabWorkspace::Sftp(sftp) = &mut tab.workspace {
                    if let Some(handle) = sftp.handle.clone() {
                        sftp.loading = true;
                        return Task::perform(sftp_list_dir(handle, path.clone()), move |result| {
                            Message::SftpDirectoryLoaded(id, path, result)
                        });
                    }
                }
            }
        }
        Message::SftpOpenEntry(id, path, is_dir) => {
            if is_dir {
                return Task::done(Message::SftpNavigate(id, path));
            }
            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                if let TabWorkspace::Sftp(sftp) = &mut tab.workspace {
                    if let Some(handle) = sftp.handle.clone() {
                        sftp.loading = true;
                        return Task::perform(
                            sftp_read_file_preview(handle, path.clone()),
                            move |result| Message::SftpFileLoaded(id, path, result),
                        );
                    }
                }
            }
        }
        Message::SftpOpenParent(id) => {
            if let Some(tab) = app.terminal_tabs.iter().find(|tab| tab.id == id) {
                if let TabWorkspace::Sftp(sftp) = &tab.workspace {
                    let current = sftp.current_path.trim_end_matches('/');
                    let parent = if current.is_empty() || current == "." || current == "/" {
                        ".".to_string()
                    } else {
                        current
                            .rsplit_once('/')
                            .map(|(parent, _)| {
                                if parent.is_empty() {
                                    "/".to_string()
                                } else {
                                    parent.to_string()
                                }
                            })
                            .unwrap_or_else(|| ".".to_string())
                    };
                    return Task::done(Message::SftpNavigate(id, parent));
                }
            }
        }
        Message::SftpRefresh(id) => {
            if let Some(tab) = app.terminal_tabs.iter().find(|tab| tab.id == id) {
                if let TabWorkspace::Sftp(sftp) = &tab.workspace {
                    return Task::done(Message::SftpNavigate(id, sftp.current_path.clone()));
                }
            }
        }
        Message::CloseDrawer => {
            app.drawer = None;
            app.context_menu = None;
            app.active_group_context = None;
            app.active_port_forward_context = None;
        }
        Message::ConnectConnection(id) => return app.open_terminal_from_connection(id),
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
        Message::TerminalSessionEvent(id, event) => {
            let mut log_message = None;

            if let Some(tab) = app.terminal_tabs.iter_mut().find(|tab| tab.id == id) {
                match event {
                    SessionEvent::Connected { description } => {
                        tab.status = description.clone();
                        log_message = Some(format!("{}: {}", tab.title, description));
                    }
                    SessionEvent::Output(bytes) => {
                        tab.terminal.feed(&bytes);
                    }
                    SessionEvent::Status(status) => {
                        tab.status = status.clone();
                        log_message = Some(format!("{}: {}", tab.title, status));
                    }
                    SessionEvent::Error(error) => {
                        tab.status = error.clone();
                        tab.terminal.push_local_line(&format!("Error: {error}"));
                        log_message = Some(format!("{}: {error}", tab.title));
                    }
                    SessionEvent::Disconnected(reason) => {
                        tab.status = reason.clone();
                        tab.terminal
                            .push_local_line(&format!("Disconnected: {reason}"));
                        tab.session = None;
                        log_message = Some(format!("{}: {reason}", tab.title));
                    }
                }
            }

            if let Some(log_message) = log_message {
                app.log(log_message);
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
                    app.prewarm_terminal_glyphs();
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
            let colors = builtin_terminal_theme_by_id("atom-one-light")
                .map(|theme| theme.colors.clone())
                .unwrap_or_else(TerminalColors::atom_one_light);
            app.settings_editor.background = colors.primary.background;
            app.settings_editor.foreground = colors.primary.foreground;
            app.settings_editor.cursor_color = colors.cursor.cursor;
            app.settings_editor.cursor_text = colors.cursor.text;
            app.settings_editor.selection_background = colors.selection.background;
            app.settings_editor.selection_foreground = colors.selection.text;
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
