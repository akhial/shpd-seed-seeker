// SPDX-License-Identifier: GPL-3.0-or-later

//! Arcane Resin is one query-wide requirement, separate from item OR groups.
//!
//! The shared editor holds it as a sheet with Arcane Resin picked: the resin
//! chip opens one on the query's condition, and a new wand's sheet hands its
//! own over when the user asks for resin instead. The dialog draws the
//! sheet's resin section and donor filters, sends each control the user
//! moves back as a [`Change`], and saves the sheet, which sets the query's
//! resin.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use shpd_seedfinder_core::editor::{Change, Draft, SaveResult, labels};

use crate::sheet::{self, Sheet};

struct Editor {
    banner: adw::Banner,
    mode: adw::ComboRow,
    explanation: adw::ActionRow,
    amount: adw::SpinRow,
    mage_wand: adw::SwitchRow,
    uncursed: adw::SwitchRow,
    limited: adw::SwitchRow,
    depth: adw::SpinRow,
    source: adw::ComboRow,
    updating: Cell<bool>,
    sheet: RefCell<Sheet>,
}

/// Presents the resin dialog over `parent` on `draft`, a sheet with Arcane
/// Resin picked. `existing` says whether the query asks for resin already,
/// which the dialog offers to remove. When the user confirms, `on_save`
/// saves the draft and answers with the editor's result; a refused save
/// keeps the dialog open on the editor's reason.
#[allow(clippy::too_many_lines)] // Declarative dialog assembly.
pub fn present(
    parent: &adw::ApplicationWindow,
    draft: Draft,
    existing: bool,
    on_save: impl Fn(&Draft) -> SaveResult + 'static,
) {
    let sheet = Sheet::new(draft);
    let title = sheet.form().title.clone();
    let editor = Rc::new(Editor {
        banner: adw::Banner::new(""),
        mode: adw::ComboRow::builder()
            .title("Minimum resin")
            .model(&gtk::StringList::new(&["Amount", labels::RESIN_AUTO]))
            .build(),
        explanation: adw::ActionRow::builder()
            .title(labels::RESIN_AUTO_TOOLTIP)
            .build(),
        amount: adw::SpinRow::builder()
            .title("Minimum resin")
            .numeric(true)
            .snap_to_ticks(true)
            .update_policy(gtk::SpinButtonUpdatePolicy::IfValid)
            .adjustment(&gtk::Adjustment::new(
                1.0,
                1.0,
                f64::from(u16::MAX),
                1.0,
                10.0,
                0.0,
            ))
            .build(),
        mage_wand: adw::SwitchRow::builder()
            .title("Include Mage’s starting wand")
            .subtitle("Add 2 resin from the Magic Missile wand recovered with Wand Preservation when imbuing another wand. The preserved wand is +0, regardless of the staff’s level.")
            .build(),
        uncursed: adw::SwitchRow::new(),
        limited: adw::SwitchRow::new(),
        depth: adw::SpinRow::builder()
            .adjustment(&gtk::Adjustment::new(1.0, 1.0, 1.0, 1.0, 1.0, 0.0))
            .build(),
        source: adw::ComboRow::builder()
            .title("Wand source")
            .model(&gtk::StringList::new(
                &sheet
                    .form()
                    .source
                    .options
                    .iter()
                    .map(|option| option.label.as_str())
                    .collect::<Vec<_>>(),
            ))
            .build(),
        updating: Cell::new(false),
        sheet: RefCell::new(sheet),
    });
    apply_form(&editor);
    connect(&editor);

    let group = adw::PreferencesGroup::new();
    group.add(&editor.mode);
    group.add(&editor.explanation);
    group.add(&editor.amount);
    group.add(&editor.mage_wand);
    group.add(&editor.uncursed);
    group.add(&editor.limited);
    group.add(&editor.depth);
    group.add(&editor.source);
    let page = adw::PreferencesPage::new();
    page.add(&group);
    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label(if existing { "Save" } else { "Add" });
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.add_top_bar(&editor.banner);
    view.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(440)
        .content_height(590)
        .child(&view)
        .build();
    dialog.set_default_widget(Some(&save));
    if existing {
        let remove = gtk::Button::builder()
            .label("Remove Arcane Resin")
            .css_classes(["destructive-action"])
            .margin_top(12)
            .build();
        group.add(&remove);
        remove.connect_clicked({
            let parent = parent.clone();
            let dialog = dialog.clone();
            move |_| {
                dialog.close();
                let _ = WidgetExt::activate_action(&parent, "win.remove-resin", None);
            }
        });
    }
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    save.connect_clicked({
        let editor = Rc::clone(&editor);
        let dialog = dialog.clone();
        move |_| {
            // A typed amount counts even before the field lets go of it.
            editor.amount.update();
            let draft = editor.sheet.borrow().draft().clone();
            match on_save(&draft) {
                SaveResult::Saved { .. } => {
                    dialog.close();
                }
                SaveResult::Refused { draft, form } => {
                    let message = form.errors.first().cloned().unwrap_or_default();
                    editor.sheet.borrow_mut().refused(draft, form);
                    apply_form(&editor);
                    editor.banner.set_title(&message);
                    editor.banner.set_revealed(true);
                }
            }
        }
    });
    dialog.present(Some(parent));
}

/// Sends every control's moves to the editor as the change it names.
fn connect(editor: &Rc<Editor>) {
    editor
        .mode
        .connect_selected_notify(hook(editor, |_, row: &adw::ComboRow| {
            Some(Change::SetResinAuto(row.selected() == 1))
        }));
    editor
        .amount
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetResinAmount(Some(row.value())))
        }));
    editor
        .mage_wand
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetIncludeMageWand(row.is_active()))
        }));
    editor
        .uncursed
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetUncursed(row.is_active()))
        }));
    editor
        .limited
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetFloorLimitEnabled(row.is_active()))
        }));
    editor
        .depth
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetFloorLimit(sheet::spin_value(row.value())))
        }));
    editor
        .source
        .connect_selected_notify(hook(editor, |editor, row: &adw::ComboRow| {
            sheet::chosen(&editor.sheet.borrow().form().source.options, row.selected())
                .map(Change::SetSource)
        }));
}

/// Wraps a control's handler: its change goes to the editor, and the dialog
/// draws the answer. Programmatic updates never re-enter, and any edit
/// retires the message of the previous save attempt.
fn hook<W>(
    editor: &Rc<Editor>,
    read: impl Fn(&Editor, &W) -> Option<Change> + 'static,
) -> impl Fn(&W) + 'static {
    let editor = Rc::clone(editor);
    move |widget| {
        if editor.updating.get() {
            return;
        }
        editor.banner.set_revealed(false);
        if let Some(change) = read(&editor, widget) {
            editor.sheet.borrow_mut().change(&change);
        }
        apply_form(&editor);
    }
}

/// Draws the sheet's resin section and donor filters.
fn apply_form(editor: &Editor) {
    let sheet = editor.sheet.borrow();
    let form = sheet.form();
    editor.updating.set(true);
    let resin = &form.resin;
    editor.mode.set_selected(u32::from(resin.auto));
    editor.explanation.set_visible(resin.auto);
    editor.amount.set_visible(!resin.auto);
    if let Some(amount) = resin.amount {
        editor.amount.set_value(amount);
    }
    editor.mage_wand.set_active(resin.include_mage_wand);
    editor.uncursed.set_title(&form.uncursed.label);
    editor.uncursed.set_active(form.uncursed.value);
    let floor = &form.floor_limit;
    editor.limited.set_title(&floor.label);
    editor.limited.set_active(floor.enabled);
    editor.depth.set_visible(floor.enabled);
    let (first, last) = sheet::floor_range(floor);
    editor.depth.adjustment().configure(
        f64::from(floor.value),
        f64::from(first),
        f64::from(last),
        1.0,
        1.0,
        0.0,
    );
    editor.depth.set_title(&floor.value_label);
    editor.source.set_selected(
        sheet::position(&form.source.options, &form.source.value)
            .unwrap_or(gtk::INVALID_LIST_POSITION),
    );
    editor.updating.set(false);
}
