// SPDX-License-Identifier: GPL-3.0-or-later

//! Arcane Resin is one query-wide requirement, separate from item OR groups.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use shpd_seedfinder_core::model::ItemSource;
use shpd_seedfinder_core::query::{ArcaneResinFilter, MAX_SEARCH_DEPTH};

use crate::state::{AppState, source_label};

pub fn summary(filter: ArcaneResinFilter) -> String {
    let mut parts = vec![
        if filter.uncursed {
            "uncursed wands"
        } else {
            "any wands"
        }
        .to_owned(),
    ];
    if let Some(depth) = filter.max_depth {
        parts.push(format!("≤ floor {depth}"));
    }
    if let Some(source) = filter.source {
        parts.push(source_label(source).to_owned());
    }
    if filter.include_mage_wand {
        parts.push("Mage +2".to_owned());
    }
    parts.join(" · ")
}

#[allow(clippy::too_many_lines)] // Declarative dialog assembly.
pub fn present(
    parent: &adw::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    refresh: &Rc<dyn Fn()>,
) {
    let snapshot = state.borrow();
    let filter = snapshot.arcane_resin_filter;
    let amount = adw::SpinRow::builder()
        .title("Minimum resin")
        .numeric(true)
        .snap_to_ticks(true)
        .update_policy(gtk::SpinButtonUpdatePolicy::IfValid)
        .adjustment(&gtk::Adjustment::new(
            f64::from(if snapshot.arcane_resin > 0 {
                snapshot.arcane_resin
            } else {
                2
            }),
            1.0,
            65535.0,
            1.0,
            10.0,
            0.0,
        ))
        .build();
    let mode = adw::ComboRow::builder()
        .title("Minimum resin")
        .model(&gtk::StringList::new(&["Amount", "Auto"]))
        .selected(u32::from(snapshot.arcane_resin_auto))
        .build();
    let explanation = adw::ActionRow::builder()
        .title("Upgrade each kept wand to +3. Excluded wands and extra copies reserved for reforging need no resin.")
        .visible(snapshot.arcane_resin_auto)
        .build();
    amount.set_visible(!snapshot.arcane_resin_auto);
    let mage_wand = adw::SwitchRow::builder()
        .title("Include Mage’s starting wand")
        .subtitle("Add 2 resin from the Magic Missile wand recovered with Wand Preservation when imbuing another wand. The preserved wand is +0, regardless of the staff’s level.")
        .active(filter.include_mage_wand)
        .build();
    let uncursed = adw::SwitchRow::builder()
        .title("Require uncursed wands")
        .active(filter.uncursed)
        .build();
    let limited = adw::SwitchRow::builder()
        .title("Limit wands to a floor")
        .active(filter.max_depth.is_some())
        .build();
    let depth = adw::SpinRow::builder()
        .title("Within first floors")
        .adjustment(&gtk::Adjustment::new(
            f64::from(filter.max_depth.unwrap_or(4)),
            1.0,
            f64::from(MAX_SEARCH_DEPTH),
            1.0,
            1.0,
            0.0,
        ))
        .visible(filter.max_depth.is_some())
        .build();
    let labels: Vec<_> = std::iter::once("Any source")
        .chain(ItemSource::ALL.iter().map(|s| source_label(*s)))
        .collect();
    let source = adw::ComboRow::builder()
        .title("Wand source")
        .model(&gtk::StringList::new(&labels))
        .selected(
            filter
                .source
                .and_then(|s| ItemSource::ALL.iter().position(|entry| *entry == s))
                .map_or(0, |index| u32::try_from(index + 1).unwrap_or(0)),
        )
        .build();
    let group = adw::PreferencesGroup::new();
    group.add(&mode);
    group.add(&explanation);
    group.add(&amount);
    group.add(&mage_wand);
    group.add(&uncursed);
    group.add(&limited);
    group.add(&depth);
    group.add(&source);
    let page = adw::PreferencesPage::new();
    page.add(&group);
    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label(if snapshot.needs_resin() {
        "Save"
    } else {
        "Add"
    });
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .title("Arcane Resin")
        .content_width(440)
        .content_height(590)
        .child(&view)
        .build();
    dialog.set_default_widget(Some(&save));
    if snapshot.needs_resin() {
        let remove = gtk::Button::builder()
            .label("Remove Arcane Resin")
            .css_classes(["destructive-action"])
            .margin_top(12)
            .build();
        group.add(&remove);
        remove.connect_clicked({
            let state = Rc::clone(state);
            let refresh = Rc::clone(refresh);
            let dialog = dialog.clone();
            move |_| {
                {
                    let mut state = state.borrow_mut();
                    state.arcane_resin = 0;
                    state.arcane_resin_auto = false;
                    state.arcane_resin_filter = ArcaneResinFilter::default();
                }
                dialog.close();
                refresh();
            }
        });
    }
    drop(snapshot);
    mode.connect_selected_notify({
        let amount = amount.clone();
        move |row| {
            amount.set_visible(row.selected() == 0);
            explanation.set_visible(row.selected() == 1);
        }
    });
    limited.connect_active_notify({
        let depth = depth.clone();
        move |row| depth.set_visible(row.is_active())
    });
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    save.connect_clicked({
        let state = Rc::clone(state);
        let refresh = Rc::clone(refresh);
        let dialog = dialog.clone();
        move |_| {
            amount.update();
            let value = amount.value();
            if mode.selected() == 0
                && (!value.is_finite() || value.fract() != 0.0 || !(1.0..=65535.0).contains(&value))
            {
                return;
            }
            {
                let mut state = state.borrow_mut();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    state.arcane_resin_auto = mode.selected() == 1;
                    state.arcane_resin = if state.arcane_resin_auto {
                        0
                    } else {
                        value as u16
                    };
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let max_depth = limited.is_active().then(|| depth.value().round() as u8);
                state.arcane_resin_filter = ArcaneResinFilter {
                    include_mage_wand: mage_wand.is_active(),
                    uncursed: uncursed.is_active(),
                    max_depth,
                    source: usize::try_from(source.selected())
                        .ok()
                        .and_then(|i| i.checked_sub(1))
                        .and_then(|i| ItemSource::ALL.get(i).copied()),
                };
            }
            dialog.close();
            refresh();
        }
    });
    dialog.present(Some(parent));
}
