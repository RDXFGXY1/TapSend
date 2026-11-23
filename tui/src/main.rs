mod app;
mod explorer;
mod server;
mod terminal;
mod ui;

use std::io;

fn main() -> io::Result<()> {
    app::App::new()?.run()
}
