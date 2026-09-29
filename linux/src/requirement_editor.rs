// SPDX-License-Identifier: GPL-3.0-or-later

//! Modal editor for one item requirement.
//!
//! The shared editor holds the sheet ([`Sheet`]) and says everything it
//! shows: which controls, their choices, ranges and words, and why a save
//! may not go through. The dialog draws that form, sends each control the
//! user moves back as a [`Change`], and draws the answer.

use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use shpd_seedfinder_core::catalog::Effect;
use shpd_seedfinder_core::editor::{
    Change, Draft, EffectControl, EffectGroup, FloorToggle, Form, FormMode, ItemChoice, ModeRange,
    Opt, RangeToggle, SaveResult, Toggle,
};

use crate::sheet::{self, Sheet};

struct Editor {
    dialog: adw::Dialog,
    heading: adw::WindowTitle,
    banner: adw::Banner,
    category: adw::ComboRow,
    item_row: adw::ComboRow,
    /// The item picker's choices, in the order its model lists them.
    items: RefCell<Vec<ItemChoice>>,
    tier_row: adw::ComboRow,
    tier_value: adw::SpinRow,
    upgrade_group: adw::PreferencesGroup,
    upgrade_row: adw::ComboRow,
    upgrade_value: adw::SpinRow,
    count_group: adw::PreferencesGroup,
    count_row: adw::SpinRow,
    copy_floor_switch: adw::SwitchRow,
    copy_floor_value: adw::SpinRow,
    levels_switch: adw::SwitchRow,
    levels_value: adw::SpinRow,
    effect_mode_group: adw::PreferencesGroup,
    effect_mode: adw::ComboRow,
    /// The "Specific…" grid, one list per heading: enchantments (or
    /// glyphs), then curses.
    effect_lists: [(EffectGroup, adw::PreferencesGroup, gtk::ListBox); 2],
    effect_checks: RefCell<Vec<(Effect, gtk::CheckButton)>>,
    details_group: adw::PreferencesGroup,
    uncursed: adw::SwitchRow,
    exclude_resin: adw::SwitchRow,
    select_trinket: adw::SwitchRow,
    allow_transmutations: adw::SwitchRow,
    transmutations: adw::SpinRow,
    source_row: adw::ComboRow,
    floor_switch: adw::SwitchRow,
    floor_value: adw::SpinRow,
    resin_group: adw::PreferencesGroup,
    resin_row: adw::ActionRow,
    updating: Cell<bool>,
    sheet: RefCell<Sheet>,
    /// The form the controls show, so a redraw rebuilds only the lists that
    /// changed: rebuilding the item list would reset its search, and the
    /// effect grid would lose the focused check box.
    shown: RefCell<Option<Form>>,
}

impl Editor {
    fn form(&self) -> Ref<'_, Form> {
        Ref::map(self.sheet.borrow(), Sheet::form)
    }
}

/// Presents the editor over `parent` on `draft`, a sheet the shared editor
/// opened. When the user confirms, `on_save` saves the draft and answers
/// with the editor's result; a refused save keeps the dialog open on the
/// editor's reasons. Where the editor offers Arcane Resin in place of a new
/// wand, the dialog's Arcane Resin row closes it and calls `on_resin`, which
/// opens the query's one resin condition — to edit it when the query has
/// one, as the resin chip does, else to add it. Cancelling calls neither.
pub fn present(
    parent: &adw::ApplicationWindow,
    draft: Draft,
    on_save: impl Fn(&Draft) -> SaveResult + 'static,
    on_resin: impl Fn() + 'static,
) -> adw::Dialog {
    let sheet = Sheet::new(draft);
    let is_new = sheet.form().mode == FormMode::New;
    let blanket = sheet.form().blanket;
    let editor = Rc::new(build(sheet));
    apply_form(&editor);
    connect(&editor);

    let title = if blanket {
        if is_new {
            "New Blanket Requirement"
        } else {
            "Edit Blanket Requirement"
        }
    } else if is_new {
        "New Requirement"
    } else {
        "Edit Requirement"
    };
    editor.dialog.set_title(title);
    editor.heading.set_title(title);
    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .title_widget(&editor.heading)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let confirm = gtk::Button::with_label(if is_new { "Add" } else { "Save" });
    confirm.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&confirm);

    let page = adw::PreferencesPage::new();
    for group in groups(&editor) {
        page.add(&group);
    }
    editor.resin_row.connect_activated({
        let dialog = editor.dialog.clone();
        move |_| {
            dialog.close();
            on_resin();
        }
    });
    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.add_top_bar(&editor.banner);
    toolbar_view.set_content(Some(&page));
    editor.dialog.set_child(Some(&toolbar_view));
    editor.dialog.set_default_widget(Some(&confirm));

    cancel.connect_clicked({
        let dialog = editor.dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    confirm.connect_clicked({
        let editor = Rc::clone(&editor);
        move |_| {
            let draft = editor.sheet.borrow().draft().clone();
            match on_save(&draft) {
                SaveResult::Saved { .. } => {
                    editor.dialog.close();
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
    editor.dialog.present(Some(parent));
    editor.dialog.clone()
}

#[allow(clippy::too_many_lines)] // Widget assembly is declarative and linear.
fn build(sheet: Sheet) -> Editor {
    let effect_list = |group| {
        let list = gtk::ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk::SelectionMode::None)
            .build();
        let preferences = adw::PreferencesGroup::new();
        preferences.add(&list);
        (group, preferences, list)
    };
    let resin_row = adw::ActionRow::builder()
        .subtitle("Require resin from surplus wands")
        .activatable(true)
        .build();
    let resin_group = adw::PreferencesGroup::new();
    resin_group.add(&resin_row);
    Editor {
        dialog: adw::Dialog::builder()
            .content_width(460)
            .content_height(700)
            .build(),
        heading: adw::WindowTitle::new("", ""),
        banner: adw::Banner::new(""),
        category: combo_row("Category"),
        item_row: searchable_combo_row("Item"),
        items: RefCell::new(Vec::new()),
        tier_row: combo_row("Tier"),
        tier_value: spin_row(""),
        upgrade_group: adw::PreferencesGroup::builder()
            .title("Upgrade level")
            .build(),
        upgrade_row: combo_row("Upgrade"),
        upgrade_value: spin_row(""),
        count_group: adw::PreferencesGroup::new(),
        count_row: spin_row("How many"),
        copy_floor_switch: adw::SwitchRow::new(),
        copy_floor_value: spin_row(""),
        levels_switch: adw::SwitchRow::new(),
        levels_value: spin_row(""),
        effect_mode_group: adw::PreferencesGroup::new(),
        effect_mode: combo_row(""),
        effect_lists: [
            effect_list(EffectGroup::Enchantment),
            effect_list(EffectGroup::Curse),
        ],
        effect_checks: RefCell::new(Vec::new()),
        details_group: adw::PreferencesGroup::builder().title("Details").build(),
        exclude_resin: adw::SwitchRow::new(),
        uncursed: adw::SwitchRow::new(),
        allow_transmutations: adw::SwitchRow::new(),
        transmutations: spin_row(""),
        select_trinket: adw::SwitchRow::new(),
        source_row: combo_row("Source"),
        floor_switch: adw::SwitchRow::new(),
        floor_value: spin_row(""),
        resin_group,
        resin_row,
        updating: Cell::new(false),
        sheet: RefCell::new(sheet),
        shown: RefCell::new(None),
    }
}

fn groups(editor: &Rc<Editor>) -> Vec<adw::PreferencesGroup> {
    let item_group = adw::PreferencesGroup::builder().title("Item").build();
    item_group.add(&editor.category);
    item_group.add(&editor.item_row);
    item_group.add(&editor.allow_transmutations);
    item_group.add(&editor.transmutations);
    item_group.add(&editor.select_trinket);
    item_group.add(&editor.tier_row);
    item_group.add(&editor.tier_value);

    editor.upgrade_group.add(&editor.upgrade_row);
    editor.upgrade_group.add(&editor.upgrade_value);

    editor.count_group.add(&editor.count_row);
    editor.count_group.add(&editor.copy_floor_switch);
    editor.count_group.add(&editor.copy_floor_value);
    editor.count_group.add(&editor.levels_switch);
    editor.count_group.add(&editor.levels_value);

    editor.effect_mode_group.add(&editor.effect_mode);

    let details_group = editor.details_group.clone();
    details_group.add(&editor.exclude_resin);
    details_group.add(&editor.uncursed);
    details_group.add(&editor.source_row);
    details_group.add(&editor.floor_switch);
    details_group.add(&editor.floor_value);

    let mut groups = vec![
        item_group,
        editor.upgrade_group.clone(),
        editor.count_group.clone(),
        editor.effect_mode_group.clone(),
    ];
    groups.extend(
        editor
            .effect_lists
            .iter()
            .map(|(_, preferences, _)| preferences.clone()),
    );
    groups.push(details_group);
    groups.push(editor.resin_group.clone());
    groups
}

/// Sends every control's moves to the editor as the change it names.
#[allow(clippy::too_many_lines)] // One handler per control.
fn connect(editor: &Rc<Editor>) {
    editor
        .category
        .connect_selected_notify(hook(editor, |editor, row: &adw::ComboRow| {
            sheet::chosen(&editor.form().kind.options, row.selected())
                .map(|kind| Change::SetKind(kind.family(), kind.weapon_category()))
        }));
    editor
        .item_row
        .connect_selected_notify(hook(editor, |editor, row: &adw::ComboRow| {
            let items = editor.items.borrow();
            let choice = items.get(usize::try_from(row.selected()).ok()?)?;
            Some(Change::SetItem(*choice))
        }));
    editor
        .tier_row
        .connect_selected_notify(hook(editor, |editor, row: &adw::ComboRow| {
            sheet::chosen(&editor.form().tier.modes, row.selected()).map(Change::SetTierMode)
        }));
    editor
        .tier_value
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetTier(sheet::spin_value(row.value())))
        }));
    editor
        .upgrade_row
        .connect_selected_notify(hook(editor, |editor, row: &adw::ComboRow| {
            sheet::chosen(&editor.form().upgrade.modes, row.selected()).map(Change::SetUpgradeMode)
        }));
    editor
        .upgrade_value
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetUpgrade(sheet::spin_value(row.value())))
        }));
    editor
        .count_row
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetCount(sheet::spin_value(row.value())))
        }));
    editor
        .copy_floor_switch
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetCopyDepthEnabled(row.is_active()))
        }));
    editor
        .copy_floor_value
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetCopyDepth(sheet::spin_value(row.value())))
        }));
    editor
        .levels_switch
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetCountLevels(row.is_active()))
        }));
    editor
        .levels_value
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetTotal(sheet::spin_value(row.value())))
        }));
    editor
        .effect_mode
        .connect_selected_notify(hook(editor, |editor, row: &adw::ComboRow| {
            sheet::chosen(&editor.form().effect.modes, row.selected()).map(Change::SetEffectMode)
        }));
    editor
        .uncursed
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetUncursed(row.is_active()))
        }));
    editor
        .exclude_resin
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetExcludeResin(row.is_active()))
        }));
    editor
        .select_trinket
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetSelectTrinket(row.is_active()))
        }));
    editor
        .allow_transmutations
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetTransmutationsEnabled(row.is_active()))
        }));
    editor
        .transmutations
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetTransmutations(sheet::spin_value(row.value())))
        }));
    editor
        .source_row
        .connect_selected_notify(hook(editor, |editor, row: &adw::ComboRow| {
            sheet::chosen(&editor.form().source.options, row.selected()).map(Change::SetSource)
        }));
    editor
        .floor_switch
        .connect_active_notify(hook(editor, |_, row: &adw::SwitchRow| {
            Some(Change::SetFloorLimitEnabled(row.is_active()))
        }));
    // The editor steps a floor limit over the empty boss floors, so the
    // spinner sends every value it lands on and shows where the editor put it.
    editor
        .floor_value
        .connect_value_notify(hook(editor, |_, row: &adw::SpinRow| {
            Some(Change::SetFloorLimit(sheet::spin_value(row.value())))
        }));
}

/// Wraps a control's handler: the change it reads from the control goes to
/// the editor, and the dialog redraws the answer — which also puts back a
/// control the editor did not follow. Programmatic updates never re-enter,
/// and any edit retires the message of the previous save attempt.
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

/// Draws the editor's current form: every control's visibility, value,
/// range and words, the choices of every picker, and the header's subtitle.
fn apply_form(editor: &Rc<Editor>) {
    let form = editor.form().clone();
    let before = editor.shown.replace(Some(form.clone()));
    let before = before.as_ref();
    editor.updating.set(true);
    editor.heading.set_subtitle(&form.title);

    fill(
        &editor.category,
        before.map(|before| &before.kind.options[..]),
        &form.kind.options,
    );
    select(
        &editor.category,
        sheet::position(&form.kind.options, &form.kind.value),
    );
    if before.is_none_or(|before| before.item.options != form.item.options) {
        let (choices, labels): (Vec<ItemChoice>, Vec<String>) =
            sheet::item_options(&form).into_iter().unzip();
        set_model(&editor.item_row, &labels);
        editor.items.replace(choices);
    }
    let item = editor
        .items
        .borrow()
        .iter()
        .position(|choice| *choice == form.item.value)
        .and_then(|index| u32::try_from(index).ok());
    select(&editor.item_row, item);
    editor
        .item_row
        .set_title(sheet::named_kind(&form).unwrap_or("Item"));
    let resin = sheet::resin_choice(&form);
    editor.resin_group.set_visible(resin.is_some());
    if let Some(resin) = resin {
        editor.resin_row.set_title(resin);
    }
    range_toggle(
        &editor.allow_transmutations,
        &editor.transmutations,
        &form.transmutations,
    );
    toggle(&editor.select_trinket, &form.select_trinket);
    mode_range(
        &editor.tier_row,
        &editor.tier_value,
        before.map(|before| &before.tier),
        &form.tier,
    );

    editor.upgrade_group.set_visible(form.upgrade.visible);
    mode_range(
        &editor.upgrade_row,
        &editor.upgrade_value,
        before.map(|before| &before.upgrade),
        &form.upgrade,
    );

    let stack = &form.stack;
    editor.count_group.set_visible(stack.visible);
    editor.count_group.set_title(&stack.label);
    set_range(&editor.count_row, stack.count, stack.min, stack.max);
    floor_toggle(
        &editor.copy_floor_switch,
        &editor.copy_floor_value,
        &stack.copy_depth,
    );
    range_toggle(
        &editor.levels_switch,
        &editor.levels_value,
        &stack.count_levels,
    );

    apply_effects(editor, before.map(|before| &before.effect), &form.effect);

    toggle(&editor.exclude_resin, &form.exclude_resin);
    toggle(&editor.uncursed, &form.uncursed);
    editor.source_row.set_visible(form.source.visible);
    fill(
        &editor.source_row,
        before.map(|before| &before.source.options[..]),
        &form.source.options,
    );
    select(
        &editor.source_row,
        sheet::position(&form.source.options, &form.source.value),
    );
    floor_toggle(&editor.floor_switch, &editor.floor_value, &form.floor_limit);
    editor.details_group.set_visible(
        form.exclude_resin.visible
            || form.uncursed.visible
            || form.source.visible
            || form.floor_limit.visible,
    );
    editor.updating.set(false);
}

/// The effect filter: its mode under the section's label, and the
/// "Specific…" grid under the editor's headings, rebuilt only when the
/// effects it lists change.
fn apply_effects(editor: &Rc<Editor>, before: Option<&EffectControl>, effect: &EffectControl) {
    editor.effect_mode_group.set_visible(effect.visible);
    editor.effect_mode_group.set_title(&effect.label);
    editor.effect_mode.set_title(&effect.label);
    fill(
        &editor.effect_mode,
        before.map(|before| &before.modes[..]),
        &effect.modes,
    );
    select(
        &editor.effect_mode,
        sheet::position(&effect.modes, &effect.mode),
    );
    if before.is_none_or(|before| !sheet::same_effects(before, effect)) {
        let mut checks = Vec::new();
        for (group, _, list) in &editor.effect_lists {
            list.remove_all();
            for choice in sheet::effect_choices(effect, *group) {
                let check = gtk::CheckButton::builder()
                    .valign(gtk::Align::Center)
                    .build();
                let row = adw::ActionRow::builder()
                    .title(&choice.label)
                    .activatable_widget(&check)
                    .build();
                row.add_prefix(&check);
                let value = choice.value;
                check.connect_toggled(hook(editor, move |_, _: &gtk::CheckButton| {
                    Some(Change::ToggleEffect(value))
                }));
                list.append(&row);
                checks.push((value, check));
            }
        }
        editor.effect_checks.replace(checks);
    }
    for (value, check) in editor.effect_checks.borrow().iter() {
        check.set_active(
            effect
                .choices
                .iter()
                .any(|choice| choice.value == *value && choice.selected),
        );
    }
    for (group, preferences, _) in &editor.effect_lists {
        let heading = sheet::effect_heading(effect, *group);
        preferences.set_visible(effect.choices_visible && heading.is_some());
        preferences.set_title(heading.unwrap_or_default());
    }
    editor.effect_lists[0]
        .1
        .set_description(Some(effect.caption.as_str()));
}

/// A check box from the editor's toggle, its help text under it.
pub(crate) fn toggle(row: &adw::SwitchRow, control: &Toggle) {
    row.set_visible(control.visible);
    row.set_title(&control.label);
    row.set_subtitle(control.caption.as_deref().unwrap_or_default());
    row.set_active(control.value);
}

/// A mode picker and the spinner of its value, shown while the editor says
/// the value shows.
fn mode_range<M: PartialEq>(
    row: &adw::ComboRow,
    spin: &adw::SpinRow,
    before: Option<&ModeRange<M>>,
    control: &ModeRange<M>,
) {
    row.set_visible(control.visible);
    fill(row, before.map(|before| &before.modes[..]), &control.modes);
    select(row, sheet::position(&control.modes, &control.mode));
    spin.set_visible(control.value_visible);
    set_range(spin, control.value, control.min, control.max);
    spin.set_title(&control.value_label);
}

/// A switch, with its help text while the editor shows it, and the spinner
/// it turns on.
fn range_toggle(switch: &adw::SwitchRow, spin: &adw::SpinRow, control: &RangeToggle) {
    switch.set_visible(control.visible);
    switch.set_title(&control.label);
    switch.set_subtitle(sheet::range_caption(control));
    switch.set_active(control.enabled);
    spin.set_visible(control.visible && control.enabled);
    set_range(spin, control.value, control.min, control.max);
    spin.set_title(&control.value_label);
}

/// A floor-limit switch and its spinner, which runs over the editor's
/// floors.
pub(crate) fn floor_toggle(switch: &adw::SwitchRow, spin: &adw::SpinRow, control: &FloorToggle) {
    switch.set_visible(control.visible);
    switch.set_title(&control.label);
    switch.set_active(control.enabled);
    spin.set_visible(control.visible && control.enabled);
    let (first, last) = sheet::floor_range(control);
    set_range(spin, control.value, first, last);
    spin.set_title(&control.value_label);
}

/// Sets a spinner's bounds and value at once, so the value is never
/// clamped to the bounds it is leaving.
fn set_range(spin: &adw::SpinRow, value: u8, min: u8, max: u8) {
    spin.adjustment().configure(
        f64::from(value),
        f64::from(min),
        f64::from(max),
        1.0,
        1.0,
        0.0,
    );
}

/// Refills a combo row when the editor's choices differ from those it
/// shows.
fn fill<T: PartialEq>(row: &adw::ComboRow, before: Option<&[Opt<T>]>, options: &[Opt<T>]) {
    if before.is_none_or(|before| before != options) {
        let labels: Vec<String> = options.iter().map(|option| option.label.clone()).collect();
        set_model(row, &labels);
    }
}

fn set_model(row: &adw::ComboRow, labels: &[String]) {
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    row.set_model(Some(&gtk::StringList::new(&labels)));
}

fn select(row: &adw::ComboRow, position: Option<u32>) {
    row.set_selected(position.unwrap_or(gtk::INVALID_LIST_POSITION));
}

fn combo_row(title: &str) -> adw::ComboRow {
    adw::ComboRow::builder().title(title).build()
}

fn searchable_combo_row(title: &str) -> adw::ComboRow {
    let row = adw::ComboRow::builder().title(title).build();
    row.set_expression(Some(&gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    )));
    row.set_enable_search(true);
    row
}

fn spin_row(title: &str) -> adw::SpinRow {
    adw::SpinRow::builder()
        .title(title)
        .adjustment(&gtk::Adjustment::new(1.0, 1.0, 1.0, 1.0, 1.0, 0.0))
        .build()
}
