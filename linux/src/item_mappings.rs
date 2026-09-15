// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::sync::LazyLock;

use adw::prelude::*;
use serde::Deserialize;
use shpd_seedfinder_core::item_mappings::item_mappings;
use shpd_seedfinder_core::seed::DungeonSeed;

use crate::sprites;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingArt {
    pub sprite_size: [i32; 2],
    pub icon_base: usize,
    pub icon_sizes: [[i32; 2]; 12],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    pub slot_size: i32,
    pub slot_gap: i32,
    pub categories: BTreeMap<String, MappingArt>,
}

pub static ARTWORK: LazyLock<Artwork> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../android/app/src/main/assets/third_party/shattered-pixel-dungeon/item-mapping-art.json"))
        .expect("bundled journal artwork must be valid")
});

pub fn present(parent: &impl IsA<gtk::Widget>, seed: DungeonSeed) -> adw::Dialog {
    let dialog = adw::Dialog::builder()
        .title("Seed information")
        .content_width(369)
        .content_height(600)
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .margin_start(16)
        .margin_end(16)
        .margin_top(12)
        .margin_bottom(16)
        .build();
    content.append(
        &gtk::Label::builder()
            .label(seed.to_code())
            .xalign(0.0)
            .css_classes(["title-3", "monospace"])
            .build(),
    );
    let detail = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .build();
    content.append(&detail);
    let mappings = item_mappings(seed);
    for (category, title, entries) in [
        ("potions", "Potions", &mappings.potions),
        ("scrolls", "Scrolls", &mappings.scrolls),
        ("rings", "Rings", &mappings.rings),
    ] {
        let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
        group.append(
            &gtk::Label::builder()
                .label(title)
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        let gap = ARTWORK.slot_gap * 3;
        let grid = gtk::Grid::builder()
            .column_spacing(gap)
            .row_spacing(gap)
            .column_homogeneous(true)
            .row_homogeneous(true)
            .halign(gtk::Align::Center)
            .build();
        for (index, entry) in entries.iter().enumerate() {
            let label = format!("{} — {}", entry.appearance, entry.name);
            let button = gtk::Button::builder()
                .child(&sprites::mapping_image(
                    entry.sprite_index,
                    &ARTWORK.categories[category],
                    index,
                ))
                .tooltip_text(&label)
                .css_classes(["flat", "mapping-tile"])
                .build();
            button.update_property(&[gtk::accessible::Property::Label(&label)]);
            let detail = detail.clone();
            button.connect_clicked(move |_| {
                let show = !detail.is_visible() || detail.text() != label;
                detail.set_label(&label);
                detail.set_visible(show);
            });
            let column = i32::try_from(index % 6).expect("six columns");
            let row = i32::try_from(index / 6).expect("two rows");
            grid.attach(&button, column, row, 1, 1);
        }
        group.append(&grid);
        content.append(&group);
    }
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&scroller));
    dialog.set_child(Some(&toolbar));
    dialog.present(Some(parent));
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut result = vec![widget.clone()];
        let mut child = widget.first_child();
        while let Some(widget) = child {
            result.extend(descendants(&widget));
            child = widget.next_sibling();
        }
        result
    }

    #[test]
    #[ignore = "requires a GTK display"]
    fn mapping_dialog_shows_six_columns_and_tap_details() {
        adw::init().unwrap();
        gtk::gio::resources_register_include!("dev.seedseeker.SeedSeeker.gresource").unwrap();
        crate::load_stylesheet();
        let window = adw::Window::builder()
            .default_width(420)
            .default_height(700)
            .build();
        window.present();
        let dialog = present(&window, DungeonSeed::from_code("ABC-DEF-GHI").unwrap());
        let settle = || {
            let context = gtk::glib::MainContext::default();
            for _ in 0..30 {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        };
        settle();
        let nodes = descendants(dialog.upcast_ref());
        let buttons = nodes
            .iter()
            .filter(|w| w.has_css_class("mapping-tile"))
            .map(|w| w.clone().downcast::<gtk::Button>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(buttons.len(), 36);
        for group in buttons.chunks(12) {
            let bounds = group
                .iter()
                .map(|b| b.compute_bounds(&dialog).unwrap())
                .collect::<Vec<_>>();
            assert!(
                bounds[..6]
                    .iter()
                    .all(|b| (b.y() - bounds[0].y()).abs() < 1.0)
            );
            assert!(
                bounds[6..]
                    .iter()
                    .all(|b| (b.y() - bounds[6].y()).abs() < 1.0)
            );
            assert!(bounds[6].y() > bounds[0].y() + bounds[0].height());
            assert!((bounds[0].x() - bounds[6].x()).abs() < 1.0);
        }
        let label = buttons[0].tooltip_text().unwrap();
        buttons[0].emit_clicked();
        settle();
        assert!(
            nodes
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .any(|w| w.is_visible() && w.text() == label)
        );
        buttons[0].emit_clicked();
        settle();
        assert!(
            !nodes
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .any(|w| w.is_visible() && w.text() == label)
        );
        let snapshot = gtk::Snapshot::new();
        gtk::WidgetPaintable::new(Some(&window)).snapshot(
            &snapshot,
            f64::from(window.width()),
            f64::from(window.height()),
        );
        let node = snapshot.to_node().unwrap();
        let texture = window.renderer().unwrap().render_texture(&node, None);
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target/gtk-native/mapping-grid.png");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        texture.save_to_png(output).unwrap();
        dialog.close();
        window.close();
    }
}
