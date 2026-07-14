// SPDX-License-Identifier: GPL-3.0-or-later

use adw::prelude::*;
use gtk::gio;

use crate::config::{APP_ID, APP_NAME};

pub fn configure(app: &adw::Application) {
    let about = gio::SimpleAction::new("about", None);
    let weak_app = app.downgrade();
    about.connect_activate(move |_, _| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };

        let dialog = adw::AboutDialog::new();
        dialog.set_application_icon(APP_ID);
        dialog.set_application_name(APP_NAME);
        dialog.set_comments("Find and inspect Shattered Pixel Dungeon seeds offline.");
        dialog.set_copyright("© 2026 Seed Seeker contributors");
        dialog.set_developer_name("Seed Seeker contributors");
        dialog.set_license_type(gtk::License::Gpl30);
        dialog.set_version(env!("CARGO_PKG_VERSION"));
        dialog.set_website("https://github.com/akhial/shpd-seed-seeker");
        dialog.present(app.active_window().as_ref());
    });
    app.add_action(&about);

    let quit = gio::SimpleAction::new("quit", None);
    let weak_app = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = weak_app.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<primary>q"]);
}
