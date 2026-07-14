// SPDX-License-Identifier: GPL-3.0-or-later

//! Seed-scout page: one seed code in, every searchable item through depth 24.

use std::fmt::Write as _;

use adw::prelude::*;
use shpd_seedfinder_core::catalog::item;
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::model::{Accessibility, ItemSource, WorldItem};
use shpd_seedfinder_core::seed::DungeonSeed;
use shpd_seedfinder_session::production_scout_world;

pub fn build(toasts: &adw::ToastOverlay) -> gtk::Widget {
    let seed_entry = gtk::Entry::builder()
        .hexpand(true)
        .input_hints(gtk::InputHints::UPPERCASE_CHARS)
        .placeholder_text("Seed code, e.g. SWL-KGN-QFD")
        .build();
    let scout_button = gtk::Button::builder()
        .css_classes(["suggested-action"])
        .label("Scout")
        .build();

    let entry_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    entry_row.append(&seed_entry);
    entry_row.append(&scout_button);

    let heading = gtk::Label::builder()
        .css_classes(["heading"])
        .label("Items through depth 24")
        .xalign(0.0)
        .build();

    let items_list = gtk::ListBox::builder()
        .css_classes(["boxed-list"])
        .selection_mode(gtk::SelectionMode::None)
        .build();
    let placeholder = gtk::Label::builder()
        .css_classes(["dim-label"])
        .label("Enter a seed code to list its searchable items")
        .margin_bottom(24)
        .margin_top(24)
        .build();
    items_list.set_placeholder(Some(&placeholder));

    let items_scroller = gtk::ScrolledWindow::builder()
        .child(&items_list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let content = gtk::Box::builder()
        .margin_bottom(12)
        .margin_end(12)
        .margin_start(12)
        .margin_top(12)
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    content.append(&entry_row);
    content.append(&heading);
    content.append(&items_scroller);

    let scout = {
        let seed_entry = seed_entry.clone();
        let items_list = items_list.clone();
        let heading = heading.clone();
        let toasts = toasts.clone();
        move || scout_into_list(&seed_entry, &items_list, &heading, &toasts)
    };
    scout_button.connect_clicked({
        let scout = scout.clone();
        move |_| scout()
    });
    seed_entry.connect_activate(move |_| scout());

    adw::Clamp::builder()
        .child(&content)
        .maximum_size(860)
        .build()
        .upcast()
}

fn scout_into_list(
    seed_entry: &gtk::Entry,
    items_list: &gtk::ListBox,
    heading: &gtk::Label,
    toasts: &adw::ToastOverlay,
) {
    let code = seed_entry.text();
    let seed = match DungeonSeed::from_code(code.trim()) {
        Ok(seed) => seed,
        Err(error) => {
            toasts.add_toast(adw::Toast::new(&format!("Invalid seed code: {error}")));
            return;
        }
    };
    let Ok(world) = production_scout_world(seed, Challenges::NONE) else {
        toasts.add_toast(adw::Toast::new(
            "World generation failed for this seed; please report it",
        ));
        return;
    };

    items_list.remove_all();
    heading.set_label(&format!(
        "Items through depth 24 — {} ({} items)",
        world.seed.to_code(),
        world.items.len()
    ));
    let mut items = world.items;
    items.sort_by_key(|world_item| world_item.depth);
    for world_item in &items {
        items_list.append(&item_row(world_item));
    }
}

fn item_row(world_item: &WorldItem) -> adw::ActionRow {
    let mut title = item(world_item.item).name.to_owned();
    if world_item.upgrade > 0 {
        let _ = write!(title, " +{}", world_item.upgrade);
    }
    if let Some(effect) = world_item.effect {
        let _ = write!(title, " · {}", effect.wire_name());
    }
    if world_item.cursed {
        title.push_str(" (cursed)");
    }
    if let Some(transmuted) = world_item.transmuted_item {
        let _ = write!(title, " → {}", item(transmuted).name);
    }

    let mut subtitle = format!(
        "Floor {} · {}",
        world_item.depth,
        source_label(world_item.source)
    );
    if world_item.accessibility != Accessibility::Independent {
        subtitle.push_str(" · mutually exclusive choice");
    }

    adw::ActionRow::builder()
        .subtitle(subtitle)
        .title(gtk::glib::markup_escape_text(&title))
        .build()
}

const fn source_label(source: ItemSource) -> &'static str {
    match source {
        ItemSource::Heap => "Floor",
        ItemSource::Chest => "Chest",
        ItemSource::LockedChest => "Locked chest",
        ItemSource::CrystalChest => "Crystal chest",
        ItemSource::Tomb => "Tomb",
        ItemSource::Skeleton => "Skeletal remains",
        ItemSource::SacrificialFire => "Sacrificial fire",
        ItemSource::Mimic => "Mimic",
        ItemSource::GoldenMimic => "Golden mimic",
        ItemSource::CrystalMimic => "Crystal mimic",
        ItemSource::Statue => "Animated statue",
        ItemSource::ArmoredStatue => "Armored statue",
        ItemSource::Shop => "Shop",
        ItemSource::GhostReward => "Sad ghost reward",
        ItemSource::WandmakerReward => "Wandmaker reward",
        ItemSource::BlacksmithReward => "Blacksmith reward",
        ItemSource::ImpReward => "Imp reward",
    }
}
