// SPDX-License-Identifier: GPL-3.0-or-later

//! The requirement sheet's side of the shared editor
//! ([`shpd_seedfinder_core::editor`]).
//!
//! The editor holds an open sheet as a draft and says everything the sheet
//! shows — which controls, their choices, ranges and words, what a save
//! would store and why it may not. The requirement and Arcane Resin dialogs
//! only draw that form and send back each control the user moved. This
//! module keeps the draft beside the form it last answered and reads the
//! pickers the way GTK's combo rows hold them, as positions in a list. It
//! holds no GTK, so its tests run anywhere.

use shpd_seedfinder_core::editor::{
    self, Change, Draft, EffectChoice, EffectControl, EffectGroup, FloorToggle, Form, ItemChoice,
    Opt,
};

/// An open sheet: the editor's draft, which the dialog never edits itself,
/// and the form the editor answered for it.
pub struct Sheet {
    draft: Draft,
    form: Form,
}

impl Sheet {
    #[must_use]
    pub fn new(draft: Draft) -> Self {
        let form = editor::form(&draft);
        Self { draft, form }
    }

    #[must_use]
    pub const fn draft(&self) -> &Draft {
        &self.draft
    }

    #[must_use]
    pub const fn form(&self) -> &Form {
        &self.form
    }

    /// Applies one control the user moved. A change the editor ignores — to
    /// a control it hides, or to a value it clamps straight back — leaves
    /// the form as it was, and the dialog draws that form again over
    /// whatever the widget shows now.
    pub fn change(&mut self, change: &Change) {
        let next = editor::change(&self.draft, change);
        if next != self.draft {
            self.form = editor::form(&next);
            self.draft = next;
        }
    }

    /// Takes the draft and form of a save the editor refused: the draft now
    /// holds the rows the save was tried on, the form says why.
    pub fn refused(&mut self, draft: Draft, form: Form) {
        self.draft = draft;
        self.form = form;
    }
}

/// Where `value` sits among a picker's options, as a combo row selects it.
#[must_use]
pub fn position<T: PartialEq>(options: &[Opt<T>], value: &T) -> Option<u32> {
    options
        .iter()
        .position(|option| option.value == *value)
        .and_then(|index| u32::try_from(index).ok())
}

/// The option a combo row's selection names.
#[must_use]
pub fn chosen<T: Copy>(options: &[Opt<T>], selected: u32) -> Option<T> {
    options
        .get(usize::try_from(selected).ok()?)
        .map(|option| option.value)
}

/// The item picker's choices as the sheet lists them: every choice the
/// editor offers, a weapon beside its tier (`Spear · Tier 2`). Arcane Resin,
/// which the editor offers among the wands, is no item here: the sheet
/// shows it as a row of its own that opens the resin dialog
/// ([`offers_resin`]).
#[must_use]
pub fn item_options(form: &Form) -> Vec<(ItemChoice, String)> {
    form.item
        .options
        .iter()
        .filter(|option| option.value != ItemChoice::ArcaneResin)
        .map(|option| {
            let label = match &option.group {
                Some(group) => format!("{} \u{b7} {group}", option.label),
                None => option.label.clone(),
            };
            (option.value, label)
        })
        .collect()
}

/// Whether the editor offers Arcane Resin in place of the item.
#[must_use]
pub fn offers_resin(form: &Form) -> bool {
    form.item
        .options
        .iter()
        .any(|option| option.value == ItemChoice::ArcaneResin)
}

/// The kind the item picker always names one of — a trinket, an artifact,
/// which have no wildcard — for its title; `None` while it offers any item.
#[must_use]
pub fn named_kind(form: &Form) -> Option<&str> {
    if form
        .item
        .options
        .iter()
        .any(|option| option.value == ItemChoice::Any)
    {
        return None;
    }
    form.kind
        .options
        .iter()
        .find(|option| option.value == form.kind.value)
        .map(|option| option.label.as_str())
}

/// The effects listed under one heading of the "Specific…" grid.
pub fn effect_choices(
    effect: &EffectControl,
    group: EffectGroup,
) -> impl Iterator<Item = &EffectChoice> {
    effect
        .choices
        .iter()
        .filter(move |choice| choice.group == group)
}

/// A heading of the "Specific…" grid (`Enchantments`, `Glyphs`, `Curses`),
/// while the grid lists it.
#[must_use]
pub fn effect_heading(effect: &EffectControl, group: EffectGroup) -> Option<&str> {
    effect
        .groups
        .iter()
        .find(|option| option.value == group)
        .map(|option| option.label.as_str())
}

/// Whether two effect grids list the same effects under the same words, so
/// only their ticks differ and the dialog keeps its check boxes.
#[must_use]
pub fn same_effects(before: &EffectControl, after: &EffectControl) -> bool {
    let layout = |effect: &EffectControl| {
        effect
            .choices
            .iter()
            .map(|choice| (choice.value, choice.group, choice.label.clone()))
            .collect::<Vec<_>>()
    };
    before.groups == after.groups && layout(before) == layout(after)
}

/// The floors a floor switch's spinner runs between: its first and last
/// choice. The editor steps over the empty boss floors in between.
#[must_use]
pub fn floor_range(toggle: &FloorToggle) -> (u8, u8) {
    let first = toggle
        .options
        .first()
        .map_or(toggle.value, |floor| floor.value);
    let last = toggle
        .options
        .last()
        .map_or(toggle.value, |floor| floor.value);
    (first, last)
}

/// A spinner's value as a change carries it: rounded and held to a byte.
/// The editor clamps it into the control's own range.
#[must_use]
pub fn spin_value(value: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Clamped just before.
    let value = value.round().clamp(0.0, f64::from(u8::MAX)) as u8;
    value
}

#[cfg(test)]
mod tests {
    use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
    use shpd_seedfinder_core::editor::{
        self, Change, EffectGroup, EffectMode, ItemChoice, KindName, SaveResult,
    };

    use super::{
        Sheet, chosen, effect_choices, effect_heading, floor_range, item_options, named_kind,
        offers_resin, position, same_effects, spin_value,
    };
    use crate::state::AppState;

    /// A new ordinary sheet on the rows `state` holds, moved control by
    /// control the way the dialog moves it.
    fn filled(state: &mut AppState, changes: &[Change]) -> Sheet {
        let mut sheet = Sheet::new(state.open_sheet(None, false));
        for change in changes {
            sheet.change(change);
        }
        sheet
    }

    #[test]
    fn pickers_read_the_form_as_combo_rows_hold_it() {
        let mut state = AppState::default();
        let sheet = filled(&mut state, &[Change::SetKind(ItemKind::Weapon, None)]);
        let form = sheet.form();
        // The kind picker lists the eight kinds; the weapon sheet starts on
        // any weapon, whose wildcard leads the item picker.
        assert_eq!(position(&form.kind.options, &form.kind.value), Some(0));
        assert_eq!(chosen(&form.kind.options, 1), Some(KindName::MeleeWeapon));
        assert_eq!(chosen(&form.kind.options, 99), None);
        let items = item_options(form);
        assert_eq!(items[0], (ItemChoice::Any, "Any weapon".to_owned()));
        assert!(items.contains(&(
            ItemChoice::Item(ItemId::Spear),
            "Spear \u{b7} Tier 2".to_owned()
        )));
        assert!(!offers_resin(form));
        assert_eq!(named_kind(form), None);

        // A new wand sheet offers Arcane Resin, but as its own row.
        let sheet = filled(&mut state, &[Change::SetKind(ItemKind::Wand, None)]);
        assert!(offers_resin(sheet.form()));
        assert!(
            item_options(sheet.form())
                .iter()
                .all(|(choice, _)| *choice != ItemChoice::ArcaneResin)
        );
        // Trinkets name one, so the picker is titled by its kind.
        let sheet = filled(&mut state, &[Change::SetKind(ItemKind::Trinket, None)]);
        assert_eq!(named_kind(sheet.form()), Some("Trinket"));
    }

    #[test]
    fn the_effect_grid_keeps_its_boxes_while_only_ticks_change() {
        let mut state = AppState::default();
        let mut sheet = filled(
            &mut state,
            &[
                Change::SetKind(ItemKind::Armor, None),
                Change::SetEffectMode(EffectMode::Specific),
            ],
        );
        let before = sheet.form().effect.clone();
        assert_eq!(
            effect_heading(&before, EffectGroup::Enchantment),
            Some("Glyphs")
        );
        assert_eq!(effect_heading(&before, EffectGroup::Curse), Some("Curses"));
        let glyph = effect_choices(&before, EffectGroup::Enchantment)
            .next()
            .unwrap()
            .value;
        sheet.change(&Change::ToggleEffect(glyph));
        let ticked = sheet.form().effect.clone();
        assert!(same_effects(&before, &ticked));
        assert!(effect_choices(&ticked, EffectGroup::Enchantment).any(|choice| choice.selected));
        // Ruling out cursed items takes the curses off the grid.
        sheet.change(&Change::SetUncursed(true));
        let uncursed = &sheet.form().effect;
        assert!(!same_effects(&ticked, uncursed));
        assert_eq!(effect_heading(uncursed, EffectGroup::Curse), None);
        assert_eq!(effect_choices(uncursed, EffectGroup::Curse).count(), 0);
    }

    #[test]
    fn floor_spinners_step_over_the_empty_boss_floors_through_the_editor() {
        let mut state = AppState::default();
        let mut sheet = filled(&mut state, &[Change::SetFloorLimitEnabled(true)]);
        let floor = &sheet.form().floor_limit;
        assert_eq!((floor.enabled, floor.value), (true, 4));
        assert_eq!(floor_range(floor), (1, 24));
        // Spinning up from 4 lands past the boss floor; typing 10 means the
        // first ten floors, which the editor reads as nine.
        sheet.change(&Change::SetFloorLimit(spin_value(5.0)));
        assert_eq!(sheet.form().floor_limit.value, 6);
        sheet.change(&Change::SetFloorLimit(spin_value(10.4)));
        let floor = &sheet.form().floor_limit;
        assert_eq!(floor.value, 9);
        assert_eq!(floor.value_label, "Within first 9 floors");
        assert_eq!(spin_value(-3.0), 0);
        assert_eq!(spin_value(700.0), 255);
    }

    #[test]
    fn a_refused_save_keeps_the_sheet_open_on_the_editors_answer() {
        let mut state = AppState::default();
        let tooth = [
            Change::SetKind(ItemKind::Trinket, None),
            Change::SetItem(ItemChoice::Item(ItemId::MimicTooth)),
        ];
        let first = filled(&mut state, &tooth);
        assert!(matches!(
            state.save(first.draft()),
            SaveResult::Saved { .. }
        ));
        let mut again = filled(&mut state, &tooth);
        // The form already says why; the save says it too, and the sheet
        // takes the draft and form it was refused with.
        assert_eq!(again.form().errors, [editor::DUPLICATE_TRINKET]);
        let SaveResult::Refused { draft, form } = state.save(again.draft()) else {
            panic!("a second Mimic Tooth is refused");
        };
        again.refused(draft, form);
        assert!(!again.form().can_save);
        assert_eq!(state.requirements.len(), 1);
    }
}
