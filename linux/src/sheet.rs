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
    Opt, RangeToggle,
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
/// ([`resin_choice`]).
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

/// The editor's name for Arcane Resin, while it offers the resin in place
/// of the item.
#[must_use]
pub fn resin_choice(form: &Form) -> Option<&str> {
    form.item
        .options
        .iter()
        .find(|option| option.value == ItemChoice::ArcaneResin)
        .map(|option| option.label.as_str())
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

/// The help text a range toggle's switch shows now, empty for none: the
/// transmutation limit's while the switch is on, the combined level's
/// whenever the switch shows — as the editor's `caption_visible` says.
#[must_use]
pub fn range_caption(control: &RangeToggle) -> &str {
    control
        .caption
        .as_deref()
        .filter(|_| control.caption_visible)
        .unwrap_or_default()
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
    use serde_json::{Value, json};
    use shpd_seedfinder_core::catalog::{Effect, ItemId, ItemKind, item, item_by_stable_id};
    use shpd_seedfinder_core::editor::{
        self, Change, ChipView, Choice, Draft, EffectGroup, EffectMode, FloorToggle, Form,
        FormMode, ItemChoice, KindName, ModeRange, Opt, Origin, RangeToggle, ResinDraft,
        ResinOutcome, SaveResult, TierMode, Toggle, UpgradeMode, labels,
    };
    use shpd_seedfinder_core::model::{ItemSource, source_name};
    use shpd_seedfinder_core::query::{ARCANE_RESIN_MAX, ARCANE_RESIN_MIN};

    use super::{
        Sheet, chosen, effect_choices, effect_heading, floor_range, item_options, named_kind,
        position, range_caption, resin_choice, same_effects, spin_value,
    };
    use crate::fixtures::{
        Fixture, decode_labelled, decode_resin, decode_rows, encode_resin, encode_row, fixtures,
        keys, tags,
    };
    use crate::state::AppState;

    // --- reading the golden requests --------------------------------------

    fn source_named(name: &str) -> ItemSource {
        *ItemSource::ALL
            .iter()
            .find(|&&source| source_name(source) == name)
            .expect("a known source")
    }

    fn item_named(name: &str) -> ItemId {
        item_by_stable_id(name).expect("a known item").id
    }

    fn small(value: &Value) -> Option<u8> {
        value.as_u64().map(|value| u8::try_from(value).unwrap())
    }

    fn effect_mode(name: &str) -> EffectMode {
        match name {
            "any" => EffectMode::Any,
            "any_enchantment" => EffectMode::AnyEnchantment,
            "specific" => EffectMode::Specific,
            other => panic!("unknown effect mode {other}"),
        }
    }

    /// The opaque draft string an envelope answered, as the typed draft it
    /// stands for.
    fn decode_draft(text: &Value) -> Draft {
        let draft: Value = serde_json::from_str(text.as_str().unwrap()).unwrap();
        let resin = &draft["resin"];
        Draft {
            v: small(&draft["v"]).unwrap(),
            origin: match draft["origin"]["type"].as_str().unwrap() {
                "new" => Origin::New,
                "row" => Origin::Row(draft["origin"]["key"].as_u64().unwrap()),
                "resin" => Origin::Resin,
                other => panic!("unknown origin {other}"),
            },
            key: draft["key"].as_u64(),
            requirement: decode_labelled(&draft["requirement"]),
            tier_value: small(&draft["tier_value"]).unwrap(),
            upgrade_value: small(&draft["upgrade_value"]).unwrap(),
            effect_mode: effect_mode(draft["effect_mode"].as_str().unwrap()),
            count: small(&draft["count"]).unwrap(),
            total: small(&draft["total"]),
            copy_depth: small(&draft["copy_depth"]),
            floor_limit_memory: small(&draft["floor_limit_memory"]).unwrap(),
            copy_depth_memory: small(&draft["copy_depth_memory"]).unwrap(),
            transmutations_memory: small(&draft["transmutations_memory"]).unwrap(),
            in_cluster: draft["in_cluster"].as_bool().unwrap(),
            blanket: draft["blanket"].as_bool().unwrap(),
            offer_resin: draft["offer_resin"].as_bool().unwrap(),
            taken_trinkets: draft["taken_trinkets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| item_named(name.as_str().unwrap()))
                .collect(),
            resin_picked: draft["resin_picked"].as_bool().unwrap(),
            resin: ResinDraft {
                auto: resin["auto"].as_bool().unwrap(),
                amount: resin["amount"].as_f64(),
                include_mage_wand: resin["include_mage_wand"].as_bool().unwrap(),
                uncursed: resin["uncursed"].as_bool().unwrap(),
                max_depth: small(&resin["max_depth"]),
                source: resin["source"].as_str().map(source_named),
            },
            query_resin: decode_resin(&draft["query_resin"]),
            rows: decode_rows(&draft["rows"]),
        }
    }

    fn decode_change(change: &Value) -> Change {
        let value = &change["value"];
        let flag = || value.as_bool().unwrap();
        let number = || small(value).unwrap();
        match change["type"].as_str().unwrap() {
            "set_kind" => {
                let kind = KindName::ALL
                    .into_iter()
                    .find(|kind| kind.name() == value)
                    .unwrap();
                Change::SetKind(kind.family(), kind.weapon_category())
            }
            "set_item" => Change::SetItem(match value.as_str() {
                None => ItemChoice::Any,
                Some("arcane_resin") => ItemChoice::ArcaneResin,
                Some(name) => ItemChoice::Item(item_named(name)),
            }),
            "toggle_effect" => Change::ToggleEffect(
                [ItemKind::Weapon, ItemKind::Armor]
                    .into_iter()
                    .find_map(|kind| Effect::from_wire_name(kind, value.as_str().unwrap()))
                    .unwrap(),
            ),
            "set_uncursed" => Change::SetUncursed(flag()),
            "set_source" => Change::SetSource(value.as_str().map(source_named)),
            "set_floor_limit_enabled" => Change::SetFloorLimitEnabled(flag()),
            "set_floor_limit" => Change::SetFloorLimit(number()),
            "set_count" => Change::SetCount(number()),
            "set_copy_depth_enabled" => Change::SetCopyDepthEnabled(flag()),
            "set_copy_depth" => Change::SetCopyDepth(number()),
            "set_count_levels" => Change::SetCountLevels(flag()),
            "set_total" => Change::SetTotal(number()),
            "set_resin_auto" => Change::SetResinAuto(flag()),
            "set_resin_amount" => Change::SetResinAmount(value.as_f64()),
            "set_include_mage_wand" => Change::SetIncludeMageWand(flag()),
            other => panic!("no Linux control sends {other}"),
        }
    }

    // --- writing the typed form as the envelope does -----------------------

    fn opt<T>(option: &Opt<T>, value: impl Fn(&T) -> Value) -> Value {
        json!({
            "value": value(&option.value),
            "label": option.label,
            "group": option.group,
            "hidden": option.hidden,
        })
    }

    fn choice<T>(control: &Choice<T>, value: impl Fn(&T) -> Value) -> Value {
        json!({
            "visible": control.visible,
            "value": value(&control.value),
            "options": control.options.iter().map(|option| opt(option, &value)).collect::<Vec<_>>(),
        })
    }

    fn mode_range<M: Copy>(control: &ModeRange<M>, name: impl Fn(M) -> &'static str) -> Value {
        json!({
            "visible": control.visible,
            "mode": name(control.mode),
            "modes": control.modes.iter().map(|option| opt(option, |mode| name(*mode).into())).collect::<Vec<_>>(),
            "value_visible": control.value_visible,
            "value": control.value,
            "min": control.min,
            "max": control.max,
            "value_label": control.value_label,
        })
    }

    fn toggle(control: &Toggle) -> Value {
        json!({
            "visible": control.visible,
            "value": control.value,
            "label": control.label,
            "caption": control.caption,
        })
    }

    fn floor_toggle(control: &FloorToggle) -> Value {
        json!({
            "visible": control.visible,
            "enabled": control.enabled,
            "value": control.value,
            "options": control.options.iter().map(|option| opt(option, |floor| (*floor).into())).collect::<Vec<_>>(),
            "label": control.label,
            "value_label": control.value_label,
        })
    }

    fn range_toggle(control: &RangeToggle) -> Value {
        json!({
            "visible": control.visible,
            "enabled": control.enabled,
            "value": control.value,
            "min": control.min,
            "max": control.max,
            "label": control.label,
            "caption": control.caption,
            "caption_visible": control.caption_visible,
            "value_label": control.value_label,
        })
    }

    fn item_choice(choice: ItemChoice) -> Value {
        match choice {
            ItemChoice::Any => Value::Null,
            ItemChoice::Item(item_id) => item(item_id).stable_id.into(),
            ItemChoice::ArcaneResin => "arcane_resin".into(),
        }
    }

    const fn tier_mode(mode: TierMode) -> &'static str {
        match mode {
            TierMode::Any => "any",
            TierMode::Exact => "exact",
            TierMode::AtLeast => "at_least",
            TierMode::AtMost => "at_most",
        }
    }

    const fn upgrade_mode(mode: UpgradeMode) -> &'static str {
        match mode {
            UpgradeMode::Any => "any",
            UpgradeMode::Exact => "exact",
            UpgradeMode::AtLeast => "at_least",
        }
    }

    const fn effect_mode_name(mode: EffectMode) -> &'static str {
        match mode {
            EffectMode::Any => "any",
            EffectMode::AnyEnchantment => "any_enchantment",
            EffectMode::Specific => "specific",
        }
    }

    const fn effect_group(group: EffectGroup) -> &'static str {
        match group {
            EffectGroup::Enchantment => "enchantment",
            EffectGroup::Curse => "curse",
        }
    }

    /// The preview chip, by the fields the envelope's chip shares with the
    /// board's.
    fn preview(chip: &ChipView) -> Value {
        json!([
            chip.key,
            chip.name,
            chip.title,
            tags(&chip.tags),
            chip.details,
            chip.description
        ])
    }

    fn expected_preview(chip: &Value) -> Value {
        if chip.is_null() {
            return Value::Null;
        }
        json!([
            chip["key"],
            chip["name"],
            chip["title"],
            chip["tags"],
            chip["details"],
            chip["description"]
        ])
    }

    /// Everything the dialogs read of a form, written as the envelope's FORM.
    fn form_value(form: &Form) -> Value {
        let effect = &form.effect;
        json!({
            "mode": match form.mode { FormMode::New => "new", FormMode::Edit => "edit" },
            "blanket": form.blanket,
            "in_cluster": form.in_cluster,
            "resin_picked": form.resin_picked,
            "title": form.title,
            "category": choice(&form.category, |kind| KindName::of(*kind, None).name().into()),
            "kind": choice(&form.kind, |kind| kind.name().into()),
            "item": choice(&form.item, |choice| item_choice(*choice)),
            "tier": mode_range(&form.tier, tier_mode),
            "upgrade": mode_range(&form.upgrade, upgrade_mode),
            "effect": {
                "visible": effect.visible,
                "label": effect.label,
                "mode": effect_mode_name(effect.mode),
                "modes": effect.modes.iter().map(|option| opt(option, |mode| effect_mode_name(*mode).into())).collect::<Vec<_>>(),
                "choices_visible": effect.choices_visible,
                "choices": effect.choices.iter().map(|choice| json!({
                    "value": choice.value.wire_name(),
                    "label": choice.label,
                    "group": effect_group(choice.group),
                    "selected": choice.selected,
                })).collect::<Vec<_>>(),
                "groups": effect.groups.iter().map(|option| opt(option, |group| effect_group(*group).into())).collect::<Vec<_>>(),
                "caption": effect.caption,
            },
            "uncursed": toggle(&form.uncursed),
            "source": choice(&form.source, |source| source.map(source_name).into()),
            "floor_limit": floor_toggle(&form.floor_limit),
            "exclude_resin": toggle(&form.exclude_resin),
            "transmutations": range_toggle(&form.transmutations),
            "select_trinket": toggle(&form.select_trinket),
            "stack": {
                "visible": form.stack.visible,
                "label": form.stack.label,
                "count": form.stack.count,
                "min": form.stack.min,
                "max": form.stack.max,
                "value_label": form.stack.value_label,
                "copy_depth": floor_toggle(&form.stack.copy_depth),
                "count_levels": range_toggle(&form.stack.count_levels),
            },
            "resin": {
                "visible": form.resin.visible,
                "label": form.resin.label,
                "auto": form.resin.auto,
                "modes": form.resin.modes.iter().map(|option| opt(option, |auto| (*auto).into())).collect::<Vec<_>>(),
                "caption": form.resin.caption,
                "amount": form.resin.amount,
                "min": form.resin.min,
                "max": form.resin.max,
                "include_mage_wand": toggle(&form.resin.include_mage_wand),
            },
            "errors": form.errors,
            "can_save": form.can_save,
        })
    }

    fn assert_form_matches(name: &str, form: &Form, expected: &Value) {
        let shown = form_value(form);
        for (field, value) in shown.as_object().unwrap() {
            assert_eq!(value, &expected[field], "{name}: form.{field}");
        }
        assert_eq!(
            form.preview.as_ref().map_or(Value::Null, preview),
            expected_preview(&expected["preview"]),
            "{name}: preview"
        );
    }

    fn open_request(request: &Value) -> Draft {
        let flag = |field: &str| request[field].as_bool().unwrap_or(false);
        editor::open(
            &decode_rows(&request["rows"]),
            request["key"].as_u64(),
            flag("blanket"),
            decode_resin(&request["resin"]).as_ref(),
            flag("offer_resin"),
            flag("open_resin"),
        )
    }

    fn assert_saved(name: &str, saved: &SaveResult, expected: &Value) {
        match saved {
            SaveResult::Saved { result, resin } => {
                let expected = &expected["saved"];
                let rows: Vec<Value> = result.rows.iter().map(encode_row).collect();
                assert_eq!(Value::Array(rows), expected["rows"], "{name}: rows");
                assert_eq!(result.next_key, expected["next_key"], "{name}: next_key");
                assert_eq!(result.changed, expected["changed"], "{name}: changed");
                assert!(result.rekeyed.is_empty(), "{name}");
                assert!(keys(&expected["rekeyed"]).is_empty(), "{name}");
                assert_eq!(json!(result.focus), expected["focus"], "{name}: focus");
                let resin = match resin {
                    ResinOutcome::Unchanged => Value::Null,
                    ResinOutcome::Set(state) => json!({ "set": encode_resin(state) }),
                    ResinOutcome::Clear => json!({ "clear": true }),
                };
                assert_eq!(resin, expected["resin"], "{name}: resin");
            }
            SaveResult::Refused { draft, form } => {
                assert_eq!(*draft, decode_draft(&expected["draft"]), "{name}: draft");
                assert_form_matches(name, form, &expected["form"]);
            }
        }
    }

    #[test]
    fn the_typed_sheet_gives_the_golden_sheet_answers_through_the_app_codec() {
        let replayed = fixtures("requirement_editor");
        assert_eq!(replayed.len(), 18);
        for Fixture {
            name,
            request,
            response,
        } in replayed
        {
            match request["op"].as_str().unwrap() {
                "open" => {
                    let sheet = Sheet::new(open_request(&request));
                    assert_eq!(*sheet.draft(), decode_draft(&response["draft"]), "{name}");
                    assert_form_matches(name, sheet.form(), &response["form"]);
                }
                "change" => {
                    let mut sheet = Sheet::new(decode_draft(&request["draft"]));
                    sheet.change(&decode_change(&request["change"]));
                    assert_eq!(*sheet.draft(), decode_draft(&response["draft"]), "{name}");
                    assert_form_matches(name, sheet.form(), &response["form"]);
                }
                "save" => {
                    let saved = editor::save(
                        &decode_draft(&request["draft"]),
                        &decode_rows(&request["rows"]),
                        request["next_key"].as_u64(),
                    );
                    assert_saved(name, &saved, &response);
                }
                other => panic!("{name}: unknown op {other}"),
            }
        }
    }

    #[test]
    fn sheet_rows_survive_the_app_codec() {
        for Fixture { name, request, .. } in fixtures("requirement_editor") {
            let rows = if request["rows"].is_null() {
                decode_draft(&request["draft"]).rows
            } else {
                decode_rows(&request["rows"])
            };
            for row in rows {
                let entry = encode_row(&row);
                assert_eq!(crate::fixtures::decode_row(&entry), row, "{name}");
            }
        }
    }

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
        assert_eq!(resin_choice(form), None);
        assert_eq!(named_kind(form), None);

        // A new wand sheet offers Arcane Resin, but as its own row.
        let sheet = filled(&mut state, &[Change::SetKind(ItemKind::Wand, None)]);
        assert_eq!(resin_choice(sheet.form()), Some("Arcane Resin"));
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
        // The grid's headings are plural, under the section's own label.
        assert_eq!(before.label, "Glyph");
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
    fn the_form_says_what_shows_and_words_every_section() {
        let mut state = AppState::default();
        // A tier's spinner shows once a mode names a tier.
        let mut sheet = filled(&mut state, &[Change::SetKind(ItemKind::Weapon, None)]);
        assert!(sheet.form().tier.visible && !sheet.form().tier.value_visible);
        sheet.change(&Change::SetTierMode(TierMode::AtLeast));
        assert!(sheet.form().tier.value_visible);
        assert!(!sheet.form().upgrade.value_visible);

        // The effect section is named for the family, and its grid shows
        // under "Specific…" alone.
        let mut sheet = filled(&mut state, &[Change::SetKind(ItemKind::Armor, None)]);
        assert_eq!(sheet.form().effect.label, "Glyph");
        assert!(!sheet.form().effect.choices_visible);
        sheet.change(&Change::SetEffectMode(EffectMode::Specific));
        assert!(sheet.form().effect.choices_visible);
        let sheet = filled(&mut state, &[Change::SetKind(ItemKind::Weapon, None)]);
        assert_eq!(sheet.form().effect.label, "Enchantment");

        // Check boxes carry their help text; the stack its label.
        let wand = filled(&mut state, &[Change::SetKind(ItemKind::Wand, None)]);
        let form = wand.form();
        assert!(form.exclude_resin.visible);
        assert!(
            form.exclude_resin
                .caption
                .as_deref()
                .is_some_and(|caption| caption.starts_with("Keep this wand"))
        );
        assert_eq!(form.uncursed.caption, None);
        assert_eq!(form.stack.label, "Total item count");

        // The transmutation limit explains itself only while it is on…
        let mut trinket = filled(&mut state, &[Change::SetKind(ItemKind::Trinket, None)]);
        let form = trinket.form();
        assert!(
            form.select_trinket
                .caption
                .as_deref()
                .is_some_and(|caption| caption.starts_with("Applies after the first brewing"))
        );
        assert!(form.transmutations.visible && !form.transmutations.enabled);
        assert_eq!(range_caption(&form.transmutations), "");
        trinket.change(&Change::SetTransmutationsEnabled(true));
        let transmutations = &trinket.form().transmutations;
        assert_eq!(
            Some(range_caption(transmutations)),
            transmutations.caption.as_deref()
        );
        assert!(range_caption(transmutations).starts_with("Matches an initial offer"));

        // …while counting levels is explained beside its switch, on or off.
        let rings = filled(
            &mut state,
            &[
                Change::SetKind(ItemKind::Ring, None),
                Change::SetItem(ItemChoice::Item(ItemId::RingMight)),
                Change::SetCount(2),
            ],
        );
        let levels = &rings.form().stack.count_levels;
        assert!(levels.visible && !levels.enabled);
        assert_eq!(
            range_caption(levels),
            "Each item counts its upgrade plus one, and spare items may go unused."
        );
    }

    #[test]
    fn the_resin_section_reads_as_the_resin_dialog_draws_it() {
        let state = AppState::default();
        let mut sheet = Sheet::new(state.open_resin());
        let resin = &sheet.form().resin;
        // The Amount/Auto picker under the section's label, as a combo row
        // holds it, and the amount field between the amounts that save.
        assert_eq!(resin.label, labels::RESIN_MINIMUM);
        let modes: Vec<&str> = resin.modes.iter().map(|mode| mode.label.as_str()).collect();
        assert_eq!(modes, [labels::RESIN_AMOUNT, labels::RESIN_AUTO]);
        assert_eq!(position(&resin.modes, &resin.auto), Some(0));
        assert_eq!((resin.min, resin.max), (ARCANE_RESIN_MIN, ARCANE_RESIN_MAX));
        let mage = &resin.include_mage_wand;
        assert!(mage.visible && !mage.value);
        assert_eq!(mage.label, labels::RESIN_MAGE_WAND);
        assert_eq!(
            mage.caption.as_deref(),
            Some(labels::RESIN_MAGE_WAND_CAPTION)
        );
        // Choosing Auto puts what it means in the amount field's place.
        sheet.change(&Change::SetResinAuto(
            chosen(&sheet.form().resin.modes, 1).unwrap(),
        ));
        let resin = &sheet.form().resin;
        assert!(resin.auto);
        assert_eq!(position(&resin.modes, &resin.auto), Some(1));
        assert_eq!(resin.caption, labels::RESIN_AUTO_CAPTION);
    }

    #[test]
    fn the_arcane_resin_row_opens_the_querys_resin() {
        // A new wand sheet offers Arcane Resin as a row of its own, which
        // opens the query's resin as the resin chip does: to add one while
        // the query has none…
        let mut state = AppState::default();
        let wand = filled(&mut state, &[Change::SetKind(ItemKind::Wand, None)]);
        assert_eq!(resin_choice(wand.form()), Some(labels::ARCANE_RESIN));
        let form = Sheet::new(state.open_resin()).form().clone();
        assert_eq!((form.mode, form.origin), (FormMode::New, Origin::New));

        // …and to edit the one it has, with Save and Remove, seeded from it.
        state.arcane_resin = 4;
        let wand = filled(&mut state, &[Change::SetKind(ItemKind::Wand, None)]);
        assert_eq!(resin_choice(wand.form()), Some(labels::ARCANE_RESIN));
        let form = Sheet::new(state.open_resin()).form().clone();
        assert_eq!((form.mode, form.origin), (FormMode::Edit, Origin::Resin));
        assert_eq!(form.resin.amount, Some(4.0));
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
