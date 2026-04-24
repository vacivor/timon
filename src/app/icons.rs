use super::*;
use iced::widget::svg;
use std::sync::OnceLock;

fn handle(bytes: &'static [u8], cache: &'static OnceLock<svg::Handle>) -> svg::Handle {
    cache
        .get_or_init(|| svg::Handle::from_memory(bytes))
        .clone()
}

fn connections_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(include_bytes!("../../assets/lucide/monitor.svg"), &HANDLE)
}

fn keychain_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(include_bytes!("../../assets/lucide/key-round.svg"), &HANDLE)
}

fn port_forwarding_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(include_bytes!("../../assets/lucide/route.svg"), &HANDLE)
}

fn snippets_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(include_bytes!("../../assets/lucide/file-text.svg"), &HANDLE)
}

fn known_hosts_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(
        include_bytes!("../../assets/lucide/shield-check.svg"),
        &HANDLE,
    )
}

fn logs_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(include_bytes!("../../assets/lucide/logs.svg"), &HANDLE)
}

fn settings_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(
        include_bytes!("../../assets/lucide/settings-2.svg"),
        &HANDLE,
    )
}

fn terminal_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(include_bytes!("../../assets/lucide/terminal.svg"), &HANDLE)
}

fn cable_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    handle(include_bytes!("../../assets/lucide/cable.svg"), &HANDLE)
}

pub(crate) fn manage_menu_icon(
    menu: ManageMenu,
    color: Color,
) -> iced::widget::svg::Svg<'static, Theme> {
    let handle = match menu {
        ManageMenu::Connections => connections_handle(),
        ManageMenu::Keychain => keychain_handle(),
        ManageMenu::PortForwarding => port_forwarding_handle(),
        ManageMenu::Snippets => snippets_handle(),
        ManageMenu::KnownHosts => known_hosts_handle(),
        ManageMenu::Logs => logs_handle(),
        ManageMenu::Settings => settings_handle(),
    };

    svg(handle)
        .width(Length::Fixed(18.0))
        .height(Length::Fixed(18.0))
        .style(move |_theme, _status| iced::widget::svg::Style { color: Some(color) })
}

pub(crate) fn connection_type_icon(
    connection_type: ConnectionType,
    color: Color,
    size: f32,
) -> iced::widget::svg::Svg<'static, Theme> {
    let handle = match connection_type {
        ConnectionType::Ssh => connections_handle(),
        ConnectionType::Local => terminal_handle(),
        ConnectionType::Serial => cable_handle(),
    };

    svg(handle)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme, _status| iced::widget::svg::Style { color: Some(color) })
}
