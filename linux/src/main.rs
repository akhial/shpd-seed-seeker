// SPDX-License-Identifier: GPL-3.0-or-later

// The search worker threads allocate thousands of short-lived buffers per
// seed; glibc's allocator is a measurable fraction of the run time.
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod application;
mod board;
mod challenges_dialog;
mod config;
mod detail_pane;
#[cfg(test)]
mod fixtures;
mod format;
mod glow;
mod item_mappings;
mod level_map_render;
mod level_map_view;
mod persist;
mod presets;
mod presets_dialog;
mod query_pane;
mod requirement_editor;
mod resin_editor;
mod result_navigation;
mod results_pane;
mod sheet;
mod sprites;
mod square;
mod state;
mod update;
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
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_startup(|_| {
        // Dark like the web and Android apps, unless the system asks for light.
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::PreferDark);
        load_stylesheet();
    });
    application::configure(&app);
    app.connect_activate(window::present);
    // A seedseeker:// share link arrives here as a `gio::File`, at cold start
    // or routed from a second invocation by GApplication's single-instance
    // handling. The window action does the decoding so it can reach the
    // window-local query state.
    app.connect_open(|app, files, _hint| {
        window::present(app);
        let Some(window) = app.active_window() else {
            return;
        };
        for file in files {
            let _ = WidgetExt::activate_action(
                &window,
                "win.open-share-link",
                Some(&file.uri().to_variant()),
            );
        }
    });
    app.run()
}

fn load_stylesheet() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_resource(&format!("{RESOURCE_BASE_PATH}/style/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    // The web's palette lies over the dark style only. GTK's
    // prefers-color-scheme media query does not follow libadwaita's
    // color scheme, so the style manager switches the sheet in and out.
    let dark = gtk::CssProvider::new();
    dark.load_from_resource(&format!("{RESOURCE_BASE_PATH}/style/style-dark.css"));
    let follow = move |manager: &adw::StyleManager| {
        if manager.is_dark() {
            gtk::style_context_add_provider_for_display(
                &display,
                &dark,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        } else {
            gtk::style_context_remove_provider_for_display(&display, &dark);
        }
    };
    let manager = adw::StyleManager::default();
    follow(&manager);
    manager.connect_dark_notify(follow);
}
