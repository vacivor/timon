#![allow(dead_code)]

mod models;
mod persistence;
mod slint_args;
mod slint_shell_app;
mod workspace;

fn main() -> anyhow::Result<()> {
    slint_shell_app::run()
}
