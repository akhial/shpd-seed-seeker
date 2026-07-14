// SPDX-License-Identifier: GPL-3.0-or-later

use adw::prelude::*;
use gtk::gio;

use crate::config::APP_NAME;
use crate::{scout_page, search_page};

pub fn present(app: &adw::Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }

    let menu = gio::Menu::new();
    menu.append(Some("About Seed Seeker"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("Main Menu")
        .build();

    let toasts = adw::ToastOverlay::new();

    let stack = adw::ViewStack::new();
    stack.add_titled_with_icon(
        &search_page::build(&toasts),
        Some("search"),
        "Search",
        "system-search-symbolic",
    );
    stack.add_titled_with_icon(
        &scout_page::build(&toasts),
        Some("scout"),
        "Scout",
        "mark-location-symbolic",
    );

    let switcher = adw::ViewSwitcher::builder()
        .policy(adw::ViewSwitcherPolicy::Wide)
        .stack(&stack)
        .build();

    let header_bar = adw::HeaderBar::new();
    header_bar.set_title_widget(Some(&switcher));
    header_bar.pack_end(&menu_button);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&stack));
    toasts.set_child(Some(&toolbar_view));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .content(&toasts)
        .default_height(640)
        .default_width(960)
        .title(APP_NAME)
        .build();
    window.present();
}
