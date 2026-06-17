#![allow(dead_code)]

mod models;
mod persistence;
mod session;
mod slint_args;
mod slint_terminal;
mod slint_terminal_app;
mod slint_terminal_core;
mod workspace;

fn main() -> anyhow::Result<()> {
    slint_terminal_app::run()
}
