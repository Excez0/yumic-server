mod audio;
mod backend;
mod config;
mod meter;
mod tray;
mod ui;

use adw::prelude::*;
use tracing::Level;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let app = adw::Application::builder()
        .application_id("io.github.yumic")
        .build();

    app.connect_activate(|app| {
        ui::build_ui(app);
    });

    app.run();
}
