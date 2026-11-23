mod app;
mod explorer;
mod server;
mod terminal;
mod ui;
#[cfg(windows)]
mod windows_console;

use std::io;

fn main() -> io::Result<()> {
    app::App::new()?.run()
}
