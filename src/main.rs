#![cfg_attr(feature = "slint-ui", allow(dead_code))]

#[cfg(all(not(feature = "iced-ui"), not(feature = "slint-ui")))]
compile_error!("Enable either the `iced-ui` feature or the `slint-ui` feature.");

#[cfg(all(feature = "iced-ui", not(feature = "slint-ui")))]
mod app;
mod models;
mod persistence;
mod session;
#[cfg(feature = "slint-ui")]
mod slint_args;
#[cfg(feature = "slint-ui")]
mod slint_shell_app;
#[cfg(feature = "slint-ui")]
mod slint_terminal;
#[cfg(feature = "slint-ui")]
mod slint_terminal_app;
#[cfg(feature = "slint-ui")]
mod slint_terminal_core;
#[cfg(all(feature = "iced-ui", not(feature = "slint-ui")))]
mod terminal;
mod workspace;

#[cfg(all(feature = "iced-ui", not(feature = "slint-ui")))]
fn main() -> iced::Result {
    app::run()
}

#[cfg(feature = "slint-ui")]
fn main() -> anyhow::Result<()> {
    match slint_ui_mode(std::env::args().skip(1)) {
        SlintUiMode::Shell => slint_shell_app::run(),
        SlintUiMode::Terminal(args) => slint_terminal_app::run_with_args(args),
    }
}

#[cfg(feature = "slint-ui")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum SlintUiMode {
    Shell,
    Terminal(Vec<String>),
}

#[cfg(feature = "slint-ui")]
fn slint_ui_mode(args: impl IntoIterator<Item = String>) -> SlintUiMode {
    let mut terminal_mode = false;
    let mut forwarded_args = Vec::new();

    for arg in args {
        if arg == slint_args::SLINT_TERMINAL_MODE_ARG {
            terminal_mode = true;
        } else {
            forwarded_args.push(arg);
        }
    }

    if terminal_mode {
        SlintUiMode::Terminal(forwarded_args)
    } else {
        SlintUiMode::Shell
    }
}

#[cfg(all(test, feature = "slint-ui"))]
mod tests {
    use super::*;

    #[test]
    fn slint_ui_mode_defaults_to_shell() {
        assert_eq!(slint_ui_mode(Vec::new()), SlintUiMode::Shell);
    }

    #[test]
    fn slint_ui_mode_strips_internal_terminal_arg() {
        assert_eq!(
            slint_ui_mode(vec![
                slint_args::SLINT_TERMINAL_MODE_ARG.into(),
                "--connection-id".into(),
                "42".into()
            ]),
            SlintUiMode::Terminal(vec!["--connection-id".into(), "42".into()])
        );
    }
}
