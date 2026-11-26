//! Clueless: a translucent meeting assistant that talks to Claude through the
//! user's claude.ai subscription.

mod audio;
mod claude;
mod config;
mod event;
mod files;
mod gate;
mod kwin;
mod question;
mod render;
mod screen;
mod stt;
mod ui;

use gtk::gio;
use gtk::prelude::*;
use gtk4 as gtk;

const APP_ID: &str = "io.github.clueless";

fn main() -> gtk::glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::default())
        .build();
    app.connect_activate(|app| match app.windows().first() {
        Some(window) => window.present(),
        None => {
            ui::build(app);
        }
    });
    // Settings are saved and child processes stopped when the window closes.
    app.run()
}
