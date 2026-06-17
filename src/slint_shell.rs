#![allow(dead_code)]

mod models;
mod persistence;
mod session;
mod slint_args;
mod slint_shell_app;
mod slint_terminal_core;
mod workspace;

fn main() -> anyhow::Result<()> {
    slint_shell_app::run()
}
