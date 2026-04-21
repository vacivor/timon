mod app;
mod models;
mod persistence;
mod session;
mod terminal;

fn main() -> iced::Result {
    app::run()
}
