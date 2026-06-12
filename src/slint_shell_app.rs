#![allow(dead_code)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use crate::models::{
    Connection, ConnectionType, Group, Identity, Key as SshKey, KnownHostEntry, ManageMenu,
    PortForward, Snippet,
};
use crate::persistence::{AppPaths, AppSettings, Database, load_settings};
use crate::workspace;
use slint::ComponentHandle;

const SHELL_LOG_LIMIT: usize = 200;
const SLINT_TERMINAL_MODE_ARG: &str = "--slint-terminal";

slint::slint! {
    import { LineEdit } from "std-widgets.slint";

    export struct ShellNavItem {
        index: int,
        title: string,
        active: bool,
    }

    export struct ShellStatItem {
        title: string,
        value: string,
        caption: string,
    }

    export struct ShellConnectionItem {
        id: string,
        name: string,
        endpoint: string,
        badge: string,
        initial: string,
        accent: color,
    }

    export struct ShellListItem {
        id: string,
        title: string,
        subtitle: string,
        badge: string,
        initial: string,
        accent: color,
    }

    component NavRow inherits Rectangle {
        in property <ShellNavItem> item;
        callback clicked();
        height: 36px;
        border-radius: 9px;
        background: item.active ? #e7edf0 : transparent;

        Rectangle {
            x: 14px;
            y: 10px;
            width: 16px;
            height: 16px;
            border-radius: 4px;
            border-width: 1px;
            border-color: item.active ? #111827 : #7b8794;
            background: item.active ? #111827 : transparent;
        }

        Text {
            x: 44px;
            y: 0px;
            width: parent.width - 54px;
            height: 36px;
            text: item.title;
            color: item.active ? #111827 : #435064;
            font-size: 14px;
            font-weight: item.active ? 700 : 500;
            vertical-alignment: center;
        }

        TouchArea {
            width: 100%;
            height: 100%;
            clicked => {
                root.clicked();
            }
        }
    }

    component StatCard inherits Rectangle {
        in property <ShellStatItem> stat;
        width: 156px;
        height: 92px;
        border-radius: 16px;
        background: #ffffff;
        border-width: 1px;
        border-color: #dde5e8;

        Text {
            x: 18px;
            y: 16px;
            width: parent.width - 36px;
            height: 18px;
            text: stat.title;
            color: #657386;
            font-size: 12px;
            font-weight: 600;
        }

        Text {
            x: 18px;
            y: 36px;
            width: parent.width - 36px;
            height: 28px;
            text: stat.value;
            color: #111827;
            font-size: 24px;
            font-weight: 800;
        }

        Text {
            x: 18px;
            y: 66px;
            width: parent.width - 36px;
            height: 18px;
            text: stat.caption;
            color: #7b8794;
            font-size: 12px;
        }
    }

    component ConnectionRow inherits Rectangle {
        in property <ShellConnectionItem> connection;
        in property <bool> selected;
        callback clicked();
        callback action(string);
        width: 360px;
        height: 60px;
        border-radius: 16px;
        background: selected ? #ffffff : #fbfcfc;
        border-width: selected ? 2px : 1px;
        border-color: selected ? #1494ff : #dfe7ea;

        Rectangle {
            x: 18px;
            y: 14px;
            width: 32px;
            height: 32px;
            border-radius: 9px;
            background: connection.accent;

            Text {
                width: 100%;
                height: 100%;
                text: connection.initial;
                color: #ffffff;
                font-size: 14px;
                font-weight: 800;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Text {
            x: 66px;
            y: 12px;
            width: parent.width - 190px;
            height: 20px;
            text: connection.name;
            color: #111827;
            font-size: 14px;
            font-weight: 700;
            overflow: elide;
        }

        Text {
            x: 66px;
            y: 32px;
            width: parent.width - 190px;
            height: 18px;
            text: connection.endpoint;
            color: #66758a;
            font-size: 12px;
            overflow: elide;
        }

        Rectangle {
            x: parent.width - 126px;
            y: 20px;
            width: 46px;
            height: 20px;
            border-radius: 10px;
            background: #eef4f6;

            Text {
                width: 100%;
                height: 100%;
                text: connection.badge;
                color: #526174;
                font-size: 10px;
                font-weight: 700;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        TouchArea {
            width: 100%;
            height: 100%;
            clicked => {
                root.clicked();
            }
        }
    }

    component ListRow inherits Rectangle {
        in property <ShellListItem> item;
        width: 360px;
        height: 60px;
        border-radius: 16px;
        background: #fbfcfc;
        border-width: 1px;
        border-color: #dfe7ea;

        Rectangle {
            x: 18px;
            y: 14px;
            width: 32px;
            height: 32px;
            border-radius: 9px;
            background: item.accent;

            Text {
                width: 100%;
                height: 100%;
                text: item.initial;
                color: #ffffff;
                font-size: 14px;
                font-weight: 800;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Text {
            x: 66px;
            y: 12px;
            width: parent.width - 150px;
            height: 20px;
            text: item.title;
            color: #111827;
            font-size: 14px;
            font-weight: 700;
            overflow: elide;
        }

        Text {
            x: 66px;
            y: 32px;
            width: parent.width - 150px;
            height: 18px;
            text: item.subtitle;
            color: #66758a;
            font-size: 12px;
            overflow: elide;
        }

        Rectangle {
            x: parent.width - 74px;
            y: 20px;
            width: 56px;
            height: 20px;
            border-radius: 10px;
            background: #eef4f6;

            Text {
                width: 100%;
                height: 100%;
                text: item.badge;
                color: #526174;
                font-size: 10px;
                font-weight: 700;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }
    }

    export component TimonSlintShellWindow inherits Window {
        in property <[ShellNavItem]> nav-items;
        in property <[ShellStatItem]> stats;
        in property <[ShellListItem]> group-items;
        in property <[ShellConnectionItem]> connections;
        in property <[ShellListItem]> list-items;
        in property <int> active-menu-index;
        in property <int> connection-group-rows;
        in property <string> page-title;
        in property <string> page-subtitle;
        in property <string> connection-count-label;
        in property <string> group-count-label;
        in property <string> selected-connection-id;
        in property <string> selected-connection-name;
        in property <string> selected-connection-endpoint;
        in property <string> selected-connection-type;
        in property <string> connect-status;
        in property <string> search-query;
        callback select-menu(int);
        callback select-connection(string);
        callback connect-selected-connection();
        callback open-connection(string);
        callback search-changed(string);

        title: "Timon";
        width: 920px;
        height: 620px;
        background: #edf1f2;

        Rectangle {
            width: 100%;
            height: 52px;
            background: #3f465c;

            Rectangle { x: 14px; y: 20px; width: 12px; height: 12px; border-radius: 6px; background: #8a91a5; }
            Rectangle { x: 34px; y: 20px; width: 12px; height: 12px; border-radius: 6px; background: #8a91a5; }
            Rectangle { x: 54px; y: 20px; width: 12px; height: 12px; border-radius: 6px; background: #8a91a5; }

            Rectangle {
                x: 84px;
                y: 10px;
                width: 132px;
                height: 32px;
                border-radius: 10px;
                background: #535a70;

                Text {
                    x: 16px;
                width: parent.width - 32px;
                height: 100%;
                    text: root.page-title;
                    color: #ffffff;
                    font-size: 13px;
                    font-weight: 700;
                    vertical-alignment: center;
                }
            }
        }

        Rectangle {
            x: 0px;
            y: 52px;
            width: 184px;
            height: parent.height - 52px;
            background: #f7f9fa;
            border-width: 0px;

            Text {
                x: 18px;
                y: 24px;
                width: parent.width - 36px;
                height: 28px;
                text: "Timon";
                color: #111827;
                font-size: 18px;
                font-weight: 800;
            }

            for item[index] in root.nav-items: NavRow {
                x: 10px;
                y: 74px + index * 44px;
                width: parent.width - 20px;
                item: item;
                clicked => {
                    root.select-menu(item.index);
                }
            }

            Rectangle {
                x: 0px;
                y: parent.height - 78px;
                width: parent.width;
                height: 1px;
                background: #dde5e8;
            }

            Text {
                x: 20px;
                y: parent.height - 56px;
                width: parent.width - 40px;
                height: 18px;
                text: "Workspace";
                color: #111827;
                font-size: 12px;
                font-weight: 700;
            }

            Text {
                x: 20px;
                y: parent.height - 36px;
                width: parent.width - 40px;
                height: 16px;
                text: root.connection-count-label + " / " + root.group-count-label;
                color: #657386;
                font-size: 11px;
            }
        }

        Rectangle {
            x: 184px;
            y: 52px;
            width: parent.width - 184px;
            height: parent.height - 52px;
            background: #edf1f2;

            Rectangle {
                x: 0px;
                y: 0px;
                width: parent.width;
                height: 56px;
                background: #e4eaec;

                LineEdit {
                    x: 18px;
                    y: 10px;
                    width: root.active-menu-index == 0 ? parent.width - 150px : parent.width - 36px;
                    height: 36px;
                    text: root.search-query;
                    placeholder-text: "Find a host or ssh user@hostname...";
                    edited(value) => {
                        root.search-changed(value);
                    }
                }

                Rectangle {
                    x: parent.width - 118px;
                    y: 12px;
                    width: 98px;
                    height: 32px;
                    border-radius: 10px;
                    background: #3f465c;
                    visible: root.active-menu-index == 0;

                    Text {
                        width: 100%;
                        height: 100%;
                        text: "CONNECT";
                        color: #ffffff;
                        font-size: 11px;
                        font-weight: 700;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }

                    TouchArea {
                        width: 100%;
                        height: 100%;
                        clicked => {
                            root.connect-selected-connection();
                        }
                    }
                }
            }

            Text {
                x: 30px;
                y: 82px;
                width: parent.width - 60px;
                height: 24px;
                text: root.page-title;
                color: #111827;
                font-size: 18px;
                font-weight: 800;
            }

            Text {
                x: 30px;
                y: 106px;
                width: parent.width - 320px;
                height: 18px;
                text: root.page-subtitle;
                color: #657386;
                font-size: 12px;
            }

            Text {
                x: parent.width - 300px;
                y: 106px;
                width: 270px;
                height: 18px;
                text: root.connect-status;
                color: #657386;
                font-size: 12px;
                horizontal-alignment: right;
                overflow: elide;
            }

            for stat[index] in root.stats: StatCard {
                x: 30px + index * 172px;
                y: 136px;
                stat: stat;
            }

            Rectangle {
                x: parent.width - 236px;
                y: 100px;
                width: 206px;
                height: 128px;
                border-radius: 16px;
                background: #ffffff;
                border-width: 1px;
                border-color: #dce5e8;
                visible: root.active-menu-index == 0;

                Text {
                    x: 18px;
                    y: 16px;
                    width: parent.width - 36px;
                    height: 18px;
                    text: "Selected";
                    color: #657386;
                    font-size: 12px;
                    font-weight: 700;
                }

                Text {
                    x: 18px;
                    y: 40px;
                    width: parent.width - 36px;
                    height: 22px;
                    text: root.selected-connection-name;
                    color: #111827;
                    font-size: 15px;
                    font-weight: 800;
                    overflow: elide;
                }

                Text {
                    x: 18px;
                    y: 66px;
                    width: parent.width - 36px;
                    height: 18px;
                    text: root.selected-connection-endpoint;
                    color: #657386;
                    font-size: 12px;
                    overflow: elide;
                }

                Rectangle {
                    x: 18px;
                    y: 94px;
                    width: 70px;
                    height: 20px;
                    border-radius: 10px;
                    background: #eef4f6;

                    Text {
                        width: 100%;
                        height: 100%;
                        text: root.selected-connection-type;
                        color: #526174;
                        font-size: 10px;
                        font-weight: 700;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                }
            }

            Text {
                x: 30px;
                y: 256px;
                width: parent.width - 60px;
                height: 22px;
                text: "Groups";
                color: #111827;
                font-size: 15px;
                font-weight: 800;
                visible: root.active-menu-index == 0;
            }

            for item[index] in root.group-items: ListRow {
                x: 30px + Math.mod(index, 2) * 378px;
                y: 292px + Math.floor(index / 2) * 74px;
                item: item;
                visible: root.active-menu-index == 0;
            }

            Text {
                x: 30px;
                y: root.active-menu-index == 0 ? 292px + root.connection-group-rows * 74px + 24px : 256px;
                width: parent.width - 60px;
                height: 22px;
                text: root.page-title;
                color: #111827;
                font-size: 15px;
                font-weight: 800;
                visible: root.active-menu-index != 0;
            }

            Text {
                x: 30px;
                y: 292px + root.connection-group-rows * 74px + 24px;
                width: parent.width - 60px;
                height: 22px;
                text: "Connections";
                color: #111827;
                font-size: 15px;
                font-weight: 800;
                visible: root.active-menu-index == 0;
            }

            for connection[index] in root.connections: ConnectionRow {
                x: 30px + Math.mod(index, 2) * 378px;
                y: 292px + root.connection-group-rows * 74px + 60px + Math.floor(index / 2) * 74px;
                connection: connection;
                selected: connection.id == root.selected-connection-id;
                visible: root.active-menu-index == 0;
                clicked => {
                    root.select-connection(connection.id);
                    root.open-connection(connection.id);
                }
            }

            for item[index] in root.list-items: ListRow {
                x: 30px + Math.mod(index, 2) * 378px;
                y: 292px + Math.floor(index / 2) * 74px;
                item: item;
                visible: root.active-menu-index != 0;
            }
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let paths = Rc::new(AppPaths::discover()?);
    let database = Rc::new(Database::new(&paths.database)?);
    let settings = Rc::new(load_settings(&paths.settings).unwrap_or_default());
    let workspace = Rc::new(RefCell::new(workspace::WorkspaceData::load(
        paths.as_ref(),
        database.as_ref(),
    )));
    let shell_logs = Rc::new(RefCell::new(initial_shell_logs(
        &workspace.borrow(),
        settings.as_ref(),
    )));

    let ui = TimonSlintShellWindow::new()?;
    apply_workspace(
        &ui,
        &workspace.borrow(),
        settings.as_ref(),
        shell_logs.borrow().as_slice(),
        ManageMenu::Connections,
        "",
    );
    {
        let workspace = workspace.borrow();
        apply_selected_connection(
            &ui,
            &workspace.connections,
            &initial_selected_connection_id(&workspace.connections),
        );
    }

    let ui_weak = ui.as_weak();
    let menu_workspace = Rc::clone(&workspace);
    let menu_settings = Rc::clone(&settings);
    let menu_logs = Rc::clone(&shell_logs);
    ui.on_select_menu(move |menu_index| {
        if let Some(ui) = ui_weak.upgrade() {
            let active_menu = manage_menu_from_index(menu_index);
            let query = ui.get_search_query().to_string();
            record_shell_log(&menu_logs, format!("Opened {}", active_menu.title()));
            let workspace = menu_workspace.borrow();
            apply_workspace(
                &ui,
                &workspace,
                menu_settings.as_ref(),
                menu_logs.borrow().as_slice(),
                active_menu,
                &query,
            );

            if active_menu == ManageMenu::Connections {
                let selected_id = filtered_selected_connection_id(
                    &workspace.groups,
                    &workspace.connections,
                    &query,
                    ui.get_selected_connection_id().as_str(),
                );
                apply_selected_connection(&ui, &workspace.connections, &selected_id);
            }
        }
    });

    let ui_weak = ui.as_weak();
    let selection_workspace = Rc::clone(&workspace);
    ui.on_select_connection(move |selected_id| {
        if let Some(ui) = ui_weak.upgrade() {
            let workspace = selection_workspace.borrow();
            apply_selected_connection(&ui, &workspace.connections, selected_id.as_str());
        }
    });

    let ui_weak = ui.as_weak();
    let connect_logs = Rc::clone(&shell_logs);
    ui.on_connect_selected_connection(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let status = launch_status_for_active_menu(
                manage_menu_from_index(ui.get_active_menu_index()),
                ui.get_selected_connection_id().as_str(),
            );
            record_shell_log(&connect_logs, status.clone());
            ui.set_connect_status(status.into());
        }
    });

    let ui_weak = ui.as_weak();
    let open_logs = Rc::clone(&shell_logs);
    ui.on_open_connection(move |connection_id| {
        if let Some(ui) = ui_weak.upgrade() {
            let status = launch_status_for_active_menu(
                manage_menu_from_index(ui.get_active_menu_index()),
                connection_id.as_str(),
            );
            record_shell_log(&open_logs, status.clone());
            ui.set_connect_status(status.into());
        }
    });

    let ui_weak = ui.as_weak();
    let search_workspace = Rc::clone(&workspace);
    let search_settings = Rc::clone(&settings);
    let search_logs = Rc::clone(&shell_logs);
    ui.on_search_changed(move |query| {
        if let Some(ui) = ui_weak.upgrade() {
            let active_menu = manage_menu_from_index(ui.get_active_menu_index());
            let workspace = search_workspace.borrow();
            apply_workspace(
                &ui,
                &workspace,
                search_settings.as_ref(),
                search_logs.borrow().as_slice(),
                active_menu,
                query.as_str(),
            );

            if active_menu == ManageMenu::Connections {
                let selected_id = filtered_selected_connection_id(
                    &workspace.groups,
                    &workspace.connections,
                    query.as_str(),
                    ui.get_selected_connection_id().as_str(),
                );
                apply_selected_connection(&ui, &workspace.connections, &selected_id);
            }
        }
    });

    ui.run()?;

    Ok(())
}

fn apply_workspace(
    ui: &TimonSlintShellWindow,
    workspace: &workspace::WorkspaceData,
    settings: &AppSettings,
    logs: &[String],
    active_menu: ManageMenu,
    search_query: &str,
) {
    ui.set_nav_items(model(nav_items(active_menu)));
    ui.set_stats(model(stat_items(workspace, settings, logs, active_menu)));
    let groups = connection_group_items(&workspace.groups, &workspace.connections, search_query);
    let group_rows = groups.len().div_ceil(2);
    ui.set_group_items(model(groups));
    ui.set_connections(model(connection_items(
        &filtered_connections(&workspace.groups, &workspace.connections, search_query),
        &workspace.groups,
    )));
    ui.set_list_items(model(list_items(
        workspace,
        settings,
        logs,
        active_menu,
        search_query,
    )));
    ui.set_active_menu_index(active_menu.index() as i32);
    ui.set_connection_group_rows(group_rows as i32);
    ui.set_page_title(active_menu.title().into());
    ui.set_page_subtitle(page_subtitle(active_menu).into());
    ui.set_search_query(search_query.into());
    ui.set_connect_status(String::new().into());
    ui.set_connection_count_label(format!("{} connections", workspace.connections.len()).into());
    ui.set_group_count_label(format!("{} groups", workspace.groups.len()).into());
}

fn launch_status_for_active_menu(active_menu: ManageMenu, selected_connection_id: &str) -> String {
    if active_menu != ManageMenu::Connections {
        return "Switch to Connections to open a terminal".into();
    }

    launch_slint_terminal_for_connection(selected_connection_id)
}

fn launch_slint_terminal_for_connection(selected_connection_id: &str) -> String {
    let Some(connection_id) = selected_connection_id.parse::<i64>().ok() else {
        return "Select a connection first".into();
    };

    let Ok(current_exe) = std::env::current_exe() else {
        return "Could not locate current executable".into();
    };
    let launch = slint_terminal_launch(&current_exe);

    let mut command = Command::new(&launch.executable);
    if let Some(mode_arg) = launch.mode_arg {
        command.arg(mode_arg);
    }

    match command
        .arg("--connection-id")
        .arg(connection_id.to_string())
        .spawn()
    {
        Ok(_) => format!("Opening connection #{connection_id}"),
        Err(error) => format!("Failed to open terminal: {error}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlintTerminalLaunch {
    executable: PathBuf,
    mode_arg: Option<&'static str>,
}

fn slint_terminal_launch(current_exe: &Path) -> SlintTerminalLaunch {
    if current_exe_is_main_timon(current_exe) {
        return SlintTerminalLaunch {
            executable: current_exe.to_path_buf(),
            mode_arg: Some(SLINT_TERMINAL_MODE_ARG),
        };
    }

    SlintTerminalLaunch {
        executable: slint_terminal_executable_path(current_exe),
        mode_arg: None,
    }
}

fn current_exe_is_main_timon(current_exe: &Path) -> bool {
    let Some(file_name) = current_exe.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    file_name.eq_ignore_ascii_case(if cfg!(windows) { "Timon.exe" } else { "Timon" })
}

fn slint_terminal_executable_path(current_exe: &Path) -> PathBuf {
    let executable_name = if cfg!(windows) {
        "TimonSlintTerminal.exe"
    } else {
        "TimonSlintTerminal"
    };

    current_exe
        .parent()
        .map(|parent| parent.join(executable_name))
        .unwrap_or_else(|| PathBuf::from(executable_name))
}

fn nav_items(active_menu: ManageMenu) -> Vec<ShellNavItem> {
    ManageMenu::ALL
        .iter()
        .map(|menu| ShellNavItem {
            index: menu.index() as i32,
            title: menu.title().into(),
            active: *menu == active_menu,
        })
        .collect()
}

fn manage_menu_from_index(index: i32) -> ManageMenu {
    ManageMenu::ALL
        .get(index.max(0) as usize)
        .copied()
        .unwrap_or(ManageMenu::Connections)
}

fn page_subtitle(active_menu: ManageMenu) -> &'static str {
    match active_menu {
        ManageMenu::Connections => "Saved SSH targets, local shells, and serial sessions.",
        ManageMenu::Keychain => "Keys and identities available to connection profiles.",
        ManageMenu::PortForwarding => "Local, remote, and dynamic forwarding rules.",
        ManageMenu::Snippets => "Reusable commands and operational playbook shortcuts.",
        ManageMenu::KnownHosts => "Hosts trusted by the SSH known_hosts store.",
        ManageMenu::Logs => "Connection and system events will be collected here.",
        ManageMenu::Settings => "Workspace-level preferences and runtime configuration.",
    }
}

fn stat_items(
    workspace: &workspace::WorkspaceData,
    settings: &AppSettings,
    logs: &[String],
    active_menu: ManageMenu,
) -> Vec<ShellStatItem> {
    match active_menu {
        ManageMenu::Connections => connection_stat_items(workspace),
        ManageMenu::Keychain => vec![
            stat_item("Keys", workspace.keys.len(), "stored keys"),
            stat_item("Identities", workspace.identities.len(), "login profiles"),
            stat_item(
                "Total",
                workspace.keys.len() + workspace.identities.len(),
                "credentials",
            ),
        ],
        ManageMenu::PortForwarding => {
            let enabled = workspace
                .port_forwards
                .iter()
                .filter(|forward| forward.enabled)
                .count();
            vec![
                stat_item("Rules", workspace.port_forwards.len(), "configured"),
                stat_item("Enabled", enabled, "active rules"),
                stat_item(
                    "Disabled",
                    workspace.port_forwards.len().saturating_sub(enabled),
                    "paused rules",
                ),
            ]
        }
        ManageMenu::KnownHosts => vec![
            stat_item(
                "Known Hosts",
                workspace.known_hosts.len(),
                "trusted entries",
            ),
            stat_item("Groups", workspace.groups.len(), "connection groups"),
            stat_item("Connections", workspace.connections.len(), "saved targets"),
        ],
        ManageMenu::Settings => vec![
            stat_text(
                "Theme",
                &settings.terminal.default_theme_id,
                "default terminal theme",
            ),
            stat_text(
                "Font",
                &format!("{:.0}px", settings.terminal.font.size),
                settings.terminal.font.family.as_str(),
            ),
            stat_item(
                "Scrollback",
                settings.terminal.scrollback_lines,
                "terminal lines",
            ),
        ],
        ManageMenu::Logs => vec![
            stat_item("Events", logs.len(), "in-memory shell logs"),
            stat_text(
                "Latest",
                logs.last().map(String::as_str).unwrap_or("No events"),
                "most recent event",
            ),
            stat_item("Limit", SHELL_LOG_LIMIT, "max entries"),
        ],
        ManageMenu::Snippets => {
            let ready = workspace
                .snippets
                .iter()
                .filter(|snippet| !snippet.command.trim().is_empty())
                .count();
            vec![
                stat_item("Snippets", workspace.snippets.len(), "saved commands"),
                stat_item("Ready", ready, "with command"),
                stat_item(
                    "Drafts",
                    workspace.snippets.len().saturating_sub(ready),
                    "missing command",
                ),
            ]
        }
    }
}

fn connection_stat_items(workspace: &workspace::WorkspaceData) -> Vec<ShellStatItem> {
    let ssh_count = workspace
        .connections
        .iter()
        .filter(|connection| connection.connection_type == ConnectionType::Ssh)
        .count();
    let local_count = workspace
        .connections
        .iter()
        .filter(|connection| connection.connection_type == ConnectionType::Local)
        .count();

    vec![
        stat_item("Connections", workspace.connections.len(), "saved targets"),
        stat_item("SSH", ssh_count, "remote hosts"),
        stat_item("Local", local_count, "shell profiles"),
    ]
}

fn stat_item(title: &str, value: usize, caption: &str) -> ShellStatItem {
    stat_text(title, &value.to_string(), caption)
}

fn stat_text(title: &str, value: &str, caption: &str) -> ShellStatItem {
    ShellStatItem {
        title: title.into(),
        value: value.into(),
        caption: caption.into(),
    }
}

fn connection_items(connections: &[Connection], groups: &[Group]) -> Vec<ShellConnectionItem> {
    connections
        .iter()
        .map(|connection| ShellConnectionItem {
            id: connection.id.to_string().into(),
            name: connection.name.clone().into(),
            endpoint: connection_summary(connection, groups).into(),
            badge: connection_type_label(connection.connection_type).into(),
            initial: connection_initial(&connection.name).into(),
            accent: connection_accent(connection.connection_type),
        })
        .collect()
}

fn connection_group_items(
    groups: &[Group],
    connections: &[Connection],
    search_query: &str,
) -> Vec<ShellListItem> {
    let items = groups
        .iter()
        .filter(|group| search_matches(search_query, [group.name.as_str()]))
        .map(|group| {
            let connection_count = connections
                .iter()
                .filter(|connection| connection.group_id == Some(group.id))
                .count();
            let parent = group
                .parent_id
                .and_then(|parent_id| groups.iter().find(|candidate| candidate.id == parent_id))
                .map(|parent| parent.name.as_str())
                .unwrap_or("Root");
            shell_list_item(
                format!("group-{}", group.id),
                group.name.clone(),
                format!("{connection_count} connections / parent {parent}"),
                "GROUP",
                connection_initial(&group.name),
                slint::Color::from_rgb_u8(0, 92, 145),
            )
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        placeholder_items("Groups", "No groups found in the current workspace.")
    } else {
        items
    }
}

fn filtered_connections(
    groups: &[Group],
    connections: &[Connection],
    search_query: &str,
) -> Vec<Connection> {
    connections
        .iter()
        .filter(|connection| {
            let endpoint = connection_endpoint(connection);
            let group_name = connection_group_name(connection, groups).unwrap_or_default();
            search_matches(
                search_query,
                [
                    connection.name.as_str(),
                    connection.host.as_str(),
                    connection.display_username.as_str(),
                    connection.username.as_str(),
                    connection.serial_port.as_str(),
                    endpoint.as_str(),
                    group_name.as_str(),
                ],
            )
        })
        .cloned()
        .collect()
}

fn filtered_selected_connection_id(
    groups: &[Group],
    connections: &[Connection],
    search_query: &str,
    current_selected_id: &str,
) -> String {
    let filtered = filtered_connections(groups, connections, search_query);
    if filtered
        .iter()
        .any(|connection| connection.id.to_string() == current_selected_id)
    {
        return current_selected_id.into();
    }

    initial_selected_connection_id(&filtered)
}

fn list_items(
    workspace: &workspace::WorkspaceData,
    settings: &AppSettings,
    logs: &[String],
    active_menu: ManageMenu,
    search_query: &str,
) -> Vec<ShellListItem> {
    let items = match active_menu {
        ManageMenu::Connections => Vec::new(),
        ManageMenu::Keychain => filter_list_items(
            keychain_items(&workspace.keys, &workspace.identities),
            search_query,
        ),
        ManageMenu::PortForwarding => {
            filter_list_items(port_forward_items(&workspace.port_forwards), search_query)
        }
        ManageMenu::KnownHosts => {
            filter_list_items(known_host_items(&workspace.known_hosts), search_query)
        }
        ManageMenu::Snippets => filter_list_items(snippet_items(&workspace.snippets), search_query),
        ManageMenu::Logs => filter_list_items(log_items(logs), search_query),
        ManageMenu::Settings => filter_list_items(settings_items(settings), search_query),
    };

    if items.is_empty() && active_menu != ManageMenu::Connections {
        return placeholder_items(
            active_menu.title(),
            "No records found in the current workspace.",
        );
    }

    items
}

fn initial_shell_logs(workspace: &workspace::WorkspaceData, settings: &AppSettings) -> Vec<String> {
    vec![
        "Timon Slint shell started".into(),
        format!(
            "Loaded {} connections, {} keys, {} identities",
            workspace.connections.len(),
            workspace.keys.len(),
            workspace.identities.len()
        ),
        format!(
            "Loaded {} port forwards and {} known hosts",
            workspace.port_forwards.len(),
            workspace.known_hosts.len()
        ),
        format!(
            "Settings loaded: theme {}, font {} {:.0}px",
            settings.terminal.default_theme_id,
            settings.terminal.font.family,
            settings.terminal.font.size
        ),
    ]
}

fn record_shell_log(logs: &Rc<RefCell<Vec<String>>>, message: impl Into<String>) {
    let mut logs = logs.borrow_mut();
    logs.push(message.into());
    if logs.len() > SHELL_LOG_LIMIT {
        let overflow = logs.len() - SHELL_LOG_LIMIT;
        logs.drain(0..overflow);
    }
}

fn shell_list_item(
    id: impl Into<slint::SharedString>,
    title: impl Into<slint::SharedString>,
    subtitle: impl Into<slint::SharedString>,
    badge: impl Into<slint::SharedString>,
    initial: impl Into<slint::SharedString>,
    accent: slint::Color,
) -> ShellListItem {
    ShellListItem {
        id: id.into(),
        title: title.into(),
        subtitle: subtitle.into(),
        badge: badge.into(),
        initial: initial.into(),
        accent,
    }
}

fn log_items(logs: &[String]) -> Vec<ShellListItem> {
    logs.iter()
        .rev()
        .enumerate()
        .map(|(index, message)| {
            let number = logs.len().saturating_sub(index);
            shell_list_item(
                format!("log-{number}"),
                format!("#{number}"),
                message.clone(),
                "LOG",
                "L",
                slint::Color::from_rgb_u8(35, 150, 165),
            )
        })
        .collect()
}

fn settings_items(settings: &AppSettings) -> Vec<ShellListItem> {
    vec![
        shell_list_item(
            "settings-font",
            "Terminal Font",
            format!(
                "{} / {:.0}px / line height {:.2}",
                settings.terminal.font.family,
                settings.terminal.font.size,
                settings.terminal.font.line_height
            ),
            "FONT",
            "F",
            slint::Color::from_rgb_u8(37, 99, 235),
        ),
        shell_list_item(
            "settings-theme",
            "Default Theme",
            settings.terminal.default_theme_id.clone(),
            "THEME",
            "T",
            slint::Color::from_rgb_u8(132, 90, 223),
        ),
        shell_list_item(
            "settings-scrollback",
            "Scrollback",
            format!("{} lines", settings.terminal.scrollback_lines),
            "TERM",
            "S",
            slint::Color::from_rgb_u8(35, 150, 165),
        ),
        shell_list_item(
            "settings-cursor",
            "Cursor",
            format!(
                "{} / {}",
                settings.terminal.cursor.shape,
                if settings.terminal.cursor.blinking {
                    "blinking"
                } else {
                    "steady"
                }
            ),
            "CURSOR",
            "C",
            slint::Color::from_rgb_u8(244, 94, 52),
        ),
        shell_list_item(
            "settings-shortcuts",
            "Shortcuts",
            format!(
                "close {} / settings {}",
                settings.shortcuts.close_tab, settings.shortcuts.open_settings
            ),
            "KEYS",
            "K",
            slint::Color::from_rgb_u8(127, 138, 150),
        ),
    ]
}

fn filter_list_items(items: Vec<ShellListItem>, search_query: &str) -> Vec<ShellListItem> {
    items
        .into_iter()
        .filter(|item| {
            search_matches(
                search_query,
                [
                    item.title.as_str(),
                    item.subtitle.as_str(),
                    item.badge.as_str(),
                ],
            )
        })
        .collect()
}

fn search_matches<'a>(search_query: &str, values: impl IntoIterator<Item = &'a str>) -> bool {
    let query = search_query.trim().to_lowercase();
    query.is_empty()
        || values
            .into_iter()
            .any(|value| value.to_lowercase().contains(&query))
}

fn keychain_items(keys: &[SshKey], identities: &[Identity]) -> Vec<ShellListItem> {
    keys.iter()
        .map(|key| {
            shell_list_item(
                format!("key-{}", key.id),
                key.name.clone(),
                key_fingerprint_preview(key),
                "KEY",
                connection_initial(&key.name),
                slint::Color::from_rgb_u8(37, 99, 235),
            )
        })
        .chain(identities.iter().map(|identity| {
            shell_list_item(
                format!("identity-{}", identity.id),
                identity.name.clone(),
                identity_subtitle(identity),
                "ID",
                connection_initial(&identity.name),
                slint::Color::from_rgb_u8(132, 90, 223),
            )
        }))
        .collect()
}

fn key_fingerprint_preview(key: &SshKey) -> String {
    if !key.public_key.trim().is_empty() {
        return key.public_key.trim().chars().take(42).collect();
    }

    if !key.certificate.trim().is_empty() {
        return "Certificate attached".into();
    }

    "Private key".into()
}

fn identity_subtitle(identity: &Identity) -> String {
    if identity.username.trim().is_empty() {
        "No username".into()
    } else {
        identity.username.trim().into()
    }
}

fn port_forward_items(port_forwards: &[PortForward]) -> Vec<ShellListItem> {
    port_forwards
        .iter()
        .map(|forward| {
            let title = if forward.label.trim().is_empty() {
                "Forward".into()
            } else {
                forward.label.clone()
            };
            shell_list_item(
                format!("port-forward-{}", forward.id),
                title,
                port_forward_subtitle(forward),
                if forward.enabled { "ON" } else { "OFF" },
                connection_initial(&forward.label),
                slint::Color::from_rgb_u8(244, 94, 52),
            )
        })
        .collect()
}

fn port_forward_subtitle(forward: &PortForward) -> String {
    format!(
        "{} {}:{} -> {}:{}",
        forward.forward_type.label(),
        forward.bind_address,
        forward.bind_port,
        forward.destination_host,
        forward.destination_port
    )
}

fn snippet_items(snippets: &[Snippet]) -> Vec<ShellListItem> {
    snippets
        .iter()
        .map(|snippet| {
            let title = if snippet.name.trim().is_empty() {
                "Snippet".into()
            } else {
                snippet.name.clone()
            };
            shell_list_item(
                snippet.id.to_string(),
                title,
                snippet_subtitle(snippet),
                "SNIP",
                connection_initial(&snippet.name),
                slint::Color::from_rgb_u8(245, 158, 11),
            )
        })
        .collect()
}

fn snippet_subtitle(snippet: &Snippet) -> String {
    let description = snippet.description.trim();
    let command = snippet.command.trim();

    match (description.is_empty(), command.is_empty()) {
        (false, false) => format!("{description} / {command}"),
        (false, true) => description.into(),
        (true, false) => command.into(),
        (true, true) => "No command".into(),
    }
}

fn known_host_items(known_hosts: &[KnownHostEntry]) -> Vec<ShellListItem> {
    known_hosts
        .iter()
        .map(|entry| {
            let title = entry
                .line
                .split_whitespace()
                .next()
                .filter(|host| !host.trim().is_empty())
                .unwrap_or("Known Host");

            shell_list_item(
                format!("known-host-{}", entry.line_number),
                title,
                format!("line {}", entry.line_number),
                "HOST",
                connection_initial(title),
                slint::Color::from_rgb_u8(35, 150, 165),
            )
        })
        .collect()
}

fn placeholder_items(title: &str, subtitle: &str) -> Vec<ShellListItem> {
    vec![shell_list_item(
        "placeholder",
        title,
        subtitle,
        "EMPTY",
        connection_initial(title),
        slint::Color::from_rgb_u8(127, 138, 150),
    )]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedConnectionDetails {
    id: String,
    name: String,
    endpoint: String,
    connection_type: String,
}

fn initial_selected_connection_id(connections: &[Connection]) -> String {
    connections
        .first()
        .map(|connection| connection.id.to_string())
        .unwrap_or_default()
}

fn selected_connection_details(
    connections: &[Connection],
    selected_id: &str,
) -> SelectedConnectionDetails {
    let selected = connections
        .iter()
        .find(|connection| connection.id.to_string() == selected_id)
        .or_else(|| connections.first());

    if let Some(connection) = selected {
        return SelectedConnectionDetails {
            id: connection.id.to_string(),
            name: connection.name.clone(),
            endpoint: connection_endpoint(connection),
            connection_type: connection_type_label(connection.connection_type).to_string(),
        };
    }

    SelectedConnectionDetails {
        id: String::new(),
        name: "No Connection".into(),
        endpoint: "Create or import a connection to get started.".into(),
        connection_type: "EMPTY".into(),
    }
}

fn apply_selected_connection(
    ui: &TimonSlintShellWindow,
    connections: &[Connection],
    selected_id: &str,
) {
    let details = selected_connection_details(connections, selected_id);

    ui.set_selected_connection_id(details.id.into());
    ui.set_selected_connection_name(details.name.into());
    ui.set_selected_connection_endpoint(details.endpoint.into());
    ui.set_selected_connection_type(details.connection_type.into());
}

fn connection_summary(connection: &Connection, groups: &[Group]) -> String {
    let endpoint = connection_endpoint(connection);
    match connection_group_name(connection, groups) {
        Some(group) => format!("{endpoint} / group {group}"),
        None => endpoint,
    }
}

fn connection_group_name(connection: &Connection, groups: &[Group]) -> Option<String> {
    connection
        .group_id
        .and_then(|group_id| groups.iter().find(|group| group.id == group_id))
        .map(|group| group.name.clone())
}

fn connection_endpoint(connection: &Connection) -> String {
    match connection.connection_type {
        ConnectionType::Local => {
            let shell = if connection.shell_path.trim().is_empty() {
                "Login Shell"
            } else {
                connection.shell_path.trim()
            };
            let work_dir = if connection.work_dir.trim().is_empty() {
                "Home"
            } else {
                connection.work_dir.trim()
            };
            format!("{shell} / {work_dir}")
        }
        ConnectionType::Serial => {
            let port = if connection.serial_port.trim().is_empty() {
                "Serial Port"
            } else {
                connection.serial_port.trim()
            };
            format!("{port} / {}", connection.baud_rate)
        }
        ConnectionType::Ssh => {
            let username = if connection.display_username.trim().is_empty() {
                "ssh"
            } else {
                connection.display_username.trim()
            };
            let host = if connection.host.trim().is_empty() {
                "hostname"
            } else {
                connection.host.trim()
            };
            format!("{username}@{host}:{}", connection.port)
        }
    }
}

fn connection_type_label(connection_type: ConnectionType) -> &'static str {
    match connection_type {
        ConnectionType::Ssh => "SSH",
        ConnectionType::Local => "LOCAL",
        ConnectionType::Serial => "SERIAL",
    }
}

fn connection_initial(name: &str) -> String {
    name.trim()
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "T".into())
}

fn connection_accent(connection_type: ConnectionType) -> slint::Color {
    match connection_type {
        ConnectionType::Ssh => slint::Color::from_rgb_u8(55, 204, 143),
        ConnectionType::Local => slint::Color::from_rgb_u8(37, 99, 235),
        ConnectionType::Serial => slint::Color::from_rgb_u8(244, 94, 52),
    }
}

fn model<T>(items: Vec<T>) -> slint::ModelRc<T>
where
    T: Clone + 'static,
{
    slint::ModelRc::new(slint::VecModel::from(items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_endpoint_uses_display_username_host_and_port() {
        let connection = Connection {
            display_username: "admin".into(),
            host: "10.10.1.110".into(),
            port: 2022,
            connection_type: ConnectionType::Ssh,
            ..Connection::default()
        };

        assert_eq!(connection_endpoint(&connection), "admin@10.10.1.110:2022");
    }

    #[test]
    fn local_endpoint_falls_back_to_login_shell_and_home() {
        let connection = Connection {
            connection_type: ConnectionType::Local,
            ..Connection::default()
        };

        assert_eq!(connection_endpoint(&connection), "Login Shell / Home");
    }

    #[test]
    fn connection_initial_uses_first_alphanumeric_character() {
        assert_eq!(connection_initial("  chore"), "C");
        assert_eq!(connection_initial("!!!"), "T");
    }

    #[test]
    fn manage_menu_from_index_falls_back_safely() {
        assert_eq!(manage_menu_from_index(1), ManageMenu::Keychain);
        assert_eq!(manage_menu_from_index(-1), ManageMenu::Connections);
        assert_eq!(manage_menu_from_index(99), ManageMenu::Connections);
    }

    #[test]
    fn slint_terminal_launch_uses_shell_sibling_for_standalone_shell() {
        let current_exe = if cfg!(windows) {
            Path::new("C:\\Timon\\TimonSlintShell.exe")
        } else {
            Path::new("/Applications/TimonSlintShell")
        };

        let launch = slint_terminal_launch(current_exe);
        let expected_name = if cfg!(windows) {
            "TimonSlintTerminal.exe"
        } else {
            "TimonSlintTerminal"
        };

        assert_eq!(
            launch.executable.file_name().and_then(|name| name.to_str()),
            Some(expected_name)
        );
        assert_eq!(launch.mode_arg, None);
    }

    #[test]
    fn slint_terminal_launch_reuses_main_timon_binary_when_available() {
        let current_exe = if cfg!(windows) {
            Path::new("C:\\Timon\\Timon.exe")
        } else {
            Path::new("/Applications/Timon")
        };

        let launch = slint_terminal_launch(current_exe);

        assert_eq!(launch.executable, current_exe);
        assert_eq!(launch.mode_arg, Some(SLINT_TERMINAL_MODE_ARG));
    }

    #[test]
    fn launch_status_rejects_non_connection_pages_before_spawning() {
        assert_eq!(
            launch_status_for_active_menu(ManageMenu::Keychain, "7"),
            "Switch to Connections to open a terminal"
        );
    }

    #[test]
    fn launch_status_rejects_missing_connection_selection_before_spawning() {
        assert_eq!(
            launch_status_for_active_menu(ManageMenu::Connections, ""),
            "Select a connection first"
        );
    }

    #[test]
    fn nav_items_marks_active_menu() {
        let items = nav_items(ManageMenu::KnownHosts);

        assert_eq!(items.len(), ManageMenu::ALL.len());
        assert!(items[ManageMenu::KnownHosts.index()].active);
        assert!(!items[ManageMenu::Connections.index()].active);
        assert_eq!(
            items[ManageMenu::KnownHosts.index()].title.to_string(),
            "Known Hosts"
        );
    }

    #[test]
    fn keychain_items_include_keys_and_identities() {
        let items = keychain_items(
            &[SshKey {
                id: 11,
                name: "prod_ed25519".into(),
                public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK".into(),
                ..SshKey::default()
            }],
            &[Identity {
                id: 22,
                name: "deploy".into(),
                username: "admin".into(),
                ..Identity::default()
            }],
        );

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.to_string(), "key-11");
        assert_eq!(items[0].title.to_string(), "prod_ed25519");
        assert_eq!(items[0].badge.to_string(), "KEY");
        assert_eq!(items[1].id.to_string(), "identity-22");
        assert_eq!(items[1].title.to_string(), "deploy");
        assert_eq!(items[1].subtitle.to_string(), "admin");
        assert_eq!(items[1].badge.to_string(), "ID");
    }

    #[test]
    fn port_forward_items_describe_route_and_enabled_state() {
        let items = port_forward_items(&[
            PortForward {
                id: 7,
                label: "redis".into(),
                enabled: true,
                bind_address: "127.0.0.1".into(),
                bind_port: 6379,
                destination_host: "10.0.0.10".into(),
                destination_port: 6379,
                ..PortForward::default()
            },
            PortForward {
                id: 8,
                label: "api".into(),
                enabled: false,
                ..PortForward::default()
            },
        ]);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.to_string(), "port-forward-7");
        assert_eq!(items[0].title.to_string(), "redis");
        assert_eq!(
            items[0].subtitle.to_string(),
            "Local 127.0.0.1:6379 -> 10.0.0.10:6379"
        );
        assert_eq!(items[0].badge.to_string(), "ON");
        assert_eq!(items[1].badge.to_string(), "OFF");
    }

    #[test]
    fn snippet_items_include_description_and_command() {
        let items = snippet_items(&[Snippet {
            id: 42,
            name: "Restart API".into(),
            description: "Production restart".into(),
            command: "systemctl restart api".into(),
        }]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id.to_string(), "42");
        assert_eq!(items[0].title.to_string(), "Restart API");
        assert_eq!(
            items[0].subtitle.to_string(),
            "Production restart / systemctl restart api"
        );
        assert_eq!(items[0].badge.to_string(), "SNIP");
    }

    #[test]
    fn snippets_page_filters_by_command_text() {
        let workspace = workspace::WorkspaceData {
            snippets: vec![
                Snippet {
                    name: "Restart API".into(),
                    command: "systemctl restart api".into(),
                    ..Snippet::default()
                },
                Snippet {
                    name: "Tail logs".into(),
                    command: "journalctl -fu timon".into(),
                    ..Snippet::default()
                },
            ],
            ..workspace::WorkspaceData::default()
        };
        let settings = AppSettings::default();
        let logs = Vec::new();

        let items = list_items(
            &workspace,
            &settings,
            &logs,
            ManageMenu::Snippets,
            "journalctl",
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Tail logs");
    }

    #[test]
    fn known_host_items_use_host_pattern_and_line_number() {
        let items = known_host_items(&[KnownHostEntry {
            line_number: 12,
            line: "example.com ssh-ed25519 AAAA".into(),
        }]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "example.com");
        assert_eq!(items[0].subtitle.to_string(), "line 12");
        assert_eq!(items[0].badge.to_string(), "HOST");
    }

    #[test]
    fn list_items_returns_empty_state_for_empty_migrated_page() {
        let workspace = workspace::WorkspaceData::default();
        let settings = AppSettings::default();
        let logs = Vec::new();
        let items = list_items(&workspace, &settings, &logs, ManageMenu::Keychain, "");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Keychain");
        assert_eq!(items[0].badge.to_string(), "EMPTY");
    }

    #[test]
    fn filtered_connections_matches_name_host_and_endpoint() {
        let groups = vec![Group {
            id: 10,
            name: "Production".into(),
            parent_id: None,
        }];
        let connections = vec![
            Connection {
                id: 1,
                name: "Local Shell".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
            Connection {
                id: 2,
                name: "Chore".into(),
                display_username: "admin".into(),
                host: "10.10.1.110".into(),
                port: 2022,
                group_id: Some(10),
                connection_type: ConnectionType::Ssh,
                ..Connection::default()
            },
        ];

        assert_eq!(
            filtered_connections(&groups, &connections, "chore").len(),
            1
        );
        assert_eq!(
            filtered_connections(&groups, &connections, "10.10.1.110")[0].id,
            2
        );
        assert_eq!(
            filtered_connections(&groups, &connections, "admin@10.10")[0].id,
            2
        );
        assert_eq!(
            filtered_connections(&groups, &connections, "production")[0].id,
            2
        );
    }

    #[test]
    fn connection_items_include_group_name_in_summary() {
        let groups = vec![Group {
            id: 10,
            name: "Production".into(),
            parent_id: None,
        }];
        let connections = vec![Connection {
            name: "Chore".into(),
            display_username: "admin".into(),
            host: "10.10.1.110".into(),
            port: 2022,
            group_id: Some(10),
            connection_type: ConnectionType::Ssh,
            ..Connection::default()
        }];

        let items = connection_items(&connections, &groups);

        assert_eq!(
            items[0].endpoint.to_string(),
            "admin@10.10.1.110:2022 / group Production"
        );
    }

    #[test]
    fn connection_group_items_include_counts_and_parent_names() {
        let groups = vec![
            Group {
                id: 1,
                name: "Production".into(),
                parent_id: None,
            },
            Group {
                id: 2,
                name: "Web".into(),
                parent_id: Some(1),
            },
        ];
        let connections = vec![
            Connection {
                group_id: Some(2),
                ..Connection::default()
            },
            Connection {
                group_id: Some(2),
                ..Connection::default()
            },
        ];

        let items = connection_group_items(&groups, &connections, "web");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Web");
        assert_eq!(
            items[0].subtitle.to_string(),
            "2 connections / parent Production"
        );
        assert_eq!(items[0].badge.to_string(), "GROUP");
    }

    #[test]
    fn connection_group_items_return_empty_state_without_groups() {
        let items = connection_group_items(&[], &[], "");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Groups");
        assert_eq!(items[0].badge.to_string(), "EMPTY");
    }

    #[test]
    fn filtered_selected_connection_falls_back_to_first_visible_result() {
        let connections = vec![
            Connection {
                id: 1,
                name: "alpha".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
            Connection {
                id: 2,
                name: "beta".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
        ];

        assert_eq!(
            filtered_selected_connection_id(&[], &connections, "beta", "1"),
            "2"
        );
        assert_eq!(
            filtered_selected_connection_id(&[], &connections, "gamma", "1"),
            ""
        );
    }

    #[test]
    fn list_items_filters_current_panel_records() {
        let workspace = workspace::WorkspaceData {
            keys: vec![SshKey {
                name: "prod_ed25519".into(),
                public_key: "ssh-ed25519 AAAAC3".into(),
                ..SshKey::default()
            }],
            identities: vec![Identity {
                name: "staging".into(),
                username: "deploy".into(),
                ..Identity::default()
            }],
            ..workspace::WorkspaceData::default()
        };
        let settings = AppSettings::default();

        let logs = Vec::new();
        let items = list_items(&workspace, &settings, &logs, ManageMenu::Keychain, "deploy");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "staging");
    }

    #[test]
    fn settings_items_include_terminal_and_shortcut_summary() {
        let mut settings = AppSettings::default();
        settings.terminal.font.family = "JetBrains Mono".into();
        settings.terminal.font.size = 13.0;
        settings.terminal.default_theme_id = "atom-one-dark".into();
        settings.shortcuts.open_settings = "Command+,".into();

        let items = settings_items(&settings);

        assert_eq!(items.len(), 5);
        assert_eq!(items[0].title.to_string(), "Terminal Font");
        assert!(items[0].subtitle.to_string().contains("JetBrains Mono"));
        assert_eq!(items[1].title.to_string(), "Default Theme");
        assert_eq!(items[1].subtitle.to_string(), "atom-one-dark");
        assert_eq!(items[4].title.to_string(), "Shortcuts");
        assert!(items[4].subtitle.to_string().contains("Command+,"));
    }

    #[test]
    fn settings_page_is_searchable() {
        let mut settings = AppSettings::default();
        settings.terminal.default_theme_id = "solarized-light".into();
        let workspace = workspace::WorkspaceData::default();

        let logs = Vec::new();
        let items = list_items(
            &workspace,
            &settings,
            &logs,
            ManageMenu::Settings,
            "solarized",
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.to_string(), "Default Theme");
    }

    #[test]
    fn initial_shell_logs_summarize_loaded_workspace() {
        let workspace = workspace::WorkspaceData {
            connections: vec![Connection::default()],
            keys: vec![SshKey::default()],
            identities: vec![Identity::default()],
            known_hosts: vec![KnownHostEntry {
                line_number: 1,
                line: "example.com ssh-ed25519 AAAA".into(),
            }],
            ..workspace::WorkspaceData::default()
        };
        let settings = AppSettings::default();

        let logs = initial_shell_logs(&workspace, &settings);

        assert!(logs[0].contains("started"));
        assert!(logs[1].contains("1 connections"));
        assert!(logs[2].contains("1 known hosts"));
    }

    #[test]
    fn record_shell_log_keeps_latest_entries_with_limit() {
        let logs = Rc::new(RefCell::new(Vec::new()));

        for index in 0..(SHELL_LOG_LIMIT + 5) {
            record_shell_log(&logs, format!("event {index}"));
        }

        let logs = logs.borrow();
        assert_eq!(logs.len(), SHELL_LOG_LIMIT);
        assert_eq!(logs.first().map(String::as_str), Some("event 5"));
        assert_eq!(logs.last().map(String::as_str), Some("event 204"));
    }

    #[test]
    fn logs_page_maps_and_filters_runtime_events() {
        let workspace = workspace::WorkspaceData::default();
        let settings = AppSettings::default();
        let logs = vec![
            "Timon Slint shell started".to_string(),
            "Opening connection #42".to_string(),
        ];

        let items = list_items(&workspace, &settings, &logs, ManageMenu::Logs, "opening");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].badge.to_string(), "LOG");
        assert!(items[0].subtitle.to_string().contains("#42"));
    }

    #[test]
    fn selected_connection_details_uses_requested_connection() {
        let connections = vec![
            Connection {
                id: 1,
                name: "Local".into(),
                connection_type: ConnectionType::Local,
                ..Connection::default()
            },
            Connection {
                id: 2,
                name: "Remote".into(),
                display_username: "admin".into(),
                host: "10.10.1.110".into(),
                port: 2022,
                connection_type: ConnectionType::Ssh,
                ..Connection::default()
            },
        ];

        assert_eq!(
            selected_connection_details(&connections, "2"),
            SelectedConnectionDetails {
                id: "2".into(),
                name: "Remote".into(),
                endpoint: "admin@10.10.1.110:2022".into(),
                connection_type: "SSH".into(),
            }
        );
    }

    #[test]
    fn selected_connection_details_falls_back_to_first_connection() {
        let connections = vec![Connection {
            id: 7,
            name: "Fallback".into(),
            connection_type: ConnectionType::Serial,
            serial_port: "/dev/tty.usbserial".into(),
            baud_rate: 9600,
            ..Connection::default()
        }];

        assert_eq!(
            selected_connection_details(&connections, "missing"),
            SelectedConnectionDetails {
                id: "7".into(),
                name: "Fallback".into(),
                endpoint: "/dev/tty.usbserial / 9600".into(),
                connection_type: "SERIAL".into(),
            }
        );
    }
}
