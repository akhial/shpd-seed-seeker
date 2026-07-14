// SPDX-License-Identifier: GPL-3.0-or-later

mod application;
mod config;
mod format;
mod scout_page;
mod search_page;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::config::{APP_ID, APP_NAME, RESOURCE_BASE_PATH};

fn main() -> glib::ExitCode {
    gio::resources_register_include!("dev.seedseeker.SeedSeeker.gresource")
        .expect("Seed Seeker resources must be valid");
    glib::set_application_name(APP_NAME);

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path(RESOURCE_BASE_PATH)
        .build();
    application::configure(&app);
    app.connect_activate(window::present);
    app.run()
}
