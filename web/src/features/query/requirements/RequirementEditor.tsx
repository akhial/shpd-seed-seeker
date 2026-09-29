import { useEffect } from "react";
import type {
  EditorChange,
  EditorFloorToggle,
  EditorOption,
  EditorSheet,
  EditorToggle,
  ItemSource,
  TierMode,
  UpgradeMode,
} from "../../../engine/types";
import { Field, Segmented, SliderRow, Sprite, Stepper } from "../../../shared/ui/primitives";
import { requirementArt } from "../../../shared/sprites/requirement-art";

// The sheet draws the shared core's form (docs/requirement-editor.md): which
// controls show, what they offer, their ranges, labels, help texts and errors
// all come from it, and every control the user moves goes back as a change.
// The dialog's own chrome — titles, the headings above pickers and mode
// pickers, slider names, button labels — is the app's.

/** What the tier slider is called in each mode; the core shows it only in a bounded one. */
const TIER_SLIDER: Record<TierMode, string> = {
  any: "Tier",
  exact: "Exact tier",
  at_least: "Minimum tier",
  at_most: "Maximum tier",
};

/** What the upgrade slider is called in each mode; the core shows it only in a bounded one. */
const UPGRADE_SLIDER: Record<UpgradeMode, string> = {
  any: "Upgrade",
  exact: "Exactly",
  at_least: "Minimum upgrade",
};

/** The item picker's options, those under one heading (`Tier 3`) in an optgroup. */
function ItemOptions({ options }: { options: EditorOption<string | null>[] }) {
  const runs: { group: string | null; options: EditorOption<string | null>[] }[] = [];
  for (const option of options) {
    const last = runs.at(-1);
    if (option.group !== null && last?.group === option.group) last.options.push(option);
    else runs.push({ group: option.group, options: [option] });
  }
  const render = (option: EditorOption<string | null>) => (
    <option key={option.value ?? ""} value={option.value ?? ""}>
      {option.label}
    </option>
  );
  return runs.flatMap(({ group, options }) =>
    group === null
      ? options.map(render)
      : [
          <optgroup key={group} label={group}>
            {options.map(render)}
          </optgroup>,
        ],
  );
}

/** A check box, with the help text the core gives it under it whenever it shows. */
function CheckBox({
  control,
  onChange,
}: {
  control: EditorToggle;
  onChange: (value: boolean) => void;
}) {
  return (
    <>
      <label className="d1-check">
        <input
          type="checkbox"
          checked={control.value}
          onChange={(event) => onChange(event.currentTarget.checked)}
        />
        <span>{control.label}</span>
      </label>
      {control.caption !== null && <p className="d1-caption">{control.caption}</p>}
    </>
  );
}

/**
 * A floor switch and its slider, which offers the floors the core lists. The
 * core's value label says the whole reading ("Within first 4 floors"), so the
 * slider keeps a fixed name of its own.
 */
function FloorLimit({
  control,
  name,
  onEnabled,
  onFloor,
}: {
  control: EditorFloorToggle;
  name: string;
  onEnabled: (enabled: boolean) => void;
  onFloor: (floor: number) => void;
}) {
  return (
    <>
      <label className="d1-check">
        <input
          type="checkbox"
          checked={control.enabled}
          onChange={(event) => onEnabled(event.currentTarget.checked)}
        />
        <span>{control.label}</span>
      </label>
      {control.enabled && (
        <SliderRow
          label={control.value_label}
          ariaLabel={name}
          values={control.options.map((option) => option.value)}
          value={control.value}
          fill
          onChange={onFloor}
        />
      )}
    </>
  );
}

export function RequirementEditor({
  sheet,
  notice,
  onChange,
  onSave,
  onCancel,
}: {
  /** The open sheet: the core's draft and the form it shows. */
  sheet: EditorSheet;
  /** Why the last request failed, when the core could not answer it. */
  notice?: string;
  onChange: (change: EditorChange) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  const { form } = sheet;
  const isNew = form.mode === "new";
  const { tier, upgrade, effect, stack, transmutations, resin } = form;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div
      className="d1-overlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onCancel();
      }}
    >
      <div
        className="d1-modal"
        role="dialog"
        aria-modal="true"
        aria-label={
          form.blanket
            ? isNew
              ? "New blanket requirement"
              : "Edit blanket requirement"
            : isNew
              ? "New requirement"
              : "Edit requirement"
        }
      >
        <header className="d1-modal-head">
          <Sprite
            art={requirementArt({ item: form.item.value, kind: form.kind.value })}
            size={28}
          />
          <div className="d1-modal-title">
            <h2>
              {form.blanket
                ? isNew
                  ? "New Blanket Requirement"
                  : "Edit Blanket Requirement"
                : isNew
                  ? "New Requirement"
                  : "Edit Requirement"}
            </h2>
            <p className="d1-mono">{form.title}</p>
          </div>
        </header>

        <div className="d1-modal-body">
          <section className="d1-modal-section">
            <h3>Item</h3>
            {form.category.visible && (
              <Segmented
                value={form.category.value}
                options={form.category.options}
                onChange={(value) => onChange({ type: "set_category", value })}
                ariaLabel="Category"
                fill
              />
            )}
            {form.weapon_type.visible && (
              <Field label="Weapon type" stack>
                <Segmented
                  value={form.weapon_type.value}
                  options={form.weapon_type.options}
                  onChange={(value) => onChange({ type: "set_weapon_type", value })}
                  ariaLabel="Weapon type"
                />
              </Field>
            )}
            {form.item.visible && (
              <Field label={form.category.value === "trinket" ? "Trinket" : "Item"}>
                <select
                  className="d1-select"
                  value={form.item.value ?? ""}
                  onChange={(event) =>
                    onChange({ type: "set_item", value: event.currentTarget.value || null })
                  }
                >
                  <ItemOptions options={form.item.options} />
                </select>
              </Field>
            )}
            {tier.visible && (
              <>
                <Field label="Tier" stack>
                  <Segmented
                    value={tier.mode}
                    options={tier.modes}
                    onChange={(value) => onChange({ type: "set_tier_mode", value })}
                    ariaLabel="Tier predicate"
                  />
                </Field>
                {tier.value_visible && (
                  <SliderRow
                    label={TIER_SLIDER[tier.mode]}
                    valueLabel={tier.value_label}
                    min={tier.min}
                    max={tier.max}
                    value={tier.value}
                    onChange={(value) => onChange({ type: "set_tier", value })}
                  />
                )}
              </>
            )}
          </section>

          {transmutations.visible && (
            <section className="d1-modal-section">
              <label className="d1-check">
                <input
                  type="checkbox"
                  checked={transmutations.enabled}
                  onChange={(event) =>
                    onChange({
                      type: "set_transmutations_enabled",
                      value: event.currentTarget.checked,
                    })
                  }
                />
                <span>{transmutations.label}</span>
              </label>
              {transmutations.enabled && (
                <Field label="Maximum transmutations" stack>
                  <Stepper
                    value={transmutations.value}
                    min={transmutations.min}
                    max={transmutations.max}
                    onChange={(value) => onChange({ type: "set_transmutations", value })}
                    ariaLabel="Maximum transmutations"
                    format={() => transmutations.value_label}
                  />
                </Field>
              )}
              {transmutations.caption_visible && transmutations.caption !== null && (
                <p className="d1-caption">{transmutations.caption}</p>
              )}
            </section>
          )}

          {form.select_trinket.visible && (
            <section className="d1-modal-section">
              <CheckBox
                control={form.select_trinket}
                onChange={(value) => onChange({ type: "set_select_trinket", value })}
              />
            </section>
          )}

          {resin.visible && (
            <section className="d1-modal-section">
              {/* The amount field takes the section's label; Auto's meaning
                  shows in its place. */}
              <Field label={resin.label} stack>
                <span className="d1-resin-amount">
                  <Segmented
                    value={resin.auto}
                    options={resin.modes}
                    onChange={(value) => onChange({ type: "set_resin_auto", value })}
                    ariaLabel="Resin amount mode"
                  />
                  {!resin.auto && (
                    <input
                      className="d1-input"
                      type="number"
                      aria-label={resin.label}
                      min={resin.min}
                      max={resin.max}
                      step={1}
                      value={resin.amount ?? ""}
                      onChange={(event) => {
                        const amount = event.currentTarget.valueAsNumber;
                        onChange({
                          type: "set_resin_amount",
                          value: Number.isNaN(amount) ? null : amount,
                        });
                      }}
                    />
                  )}
                </span>
              </Field>
              {resin.auto && <p className="d1-caption">{resin.caption}</p>}
            </section>
          )}

          {upgrade.visible && (
            <section className="d1-modal-section">
              <h3>Upgrade level</h3>
              <Segmented
                value={upgrade.mode}
                options={upgrade.modes}
                onChange={(value) => onChange({ type: "set_upgrade_mode", value })}
                ariaLabel="Upgrade predicate"
                fill
              />
              {upgrade.value_visible && (
                <SliderRow
                  label={UPGRADE_SLIDER[upgrade.mode]}
                  valueLabel={upgrade.value_label}
                  min={upgrade.min}
                  max={upgrade.max}
                  value={upgrade.value}
                  onChange={(value) => onChange({ type: "set_upgrade", value })}
                />
              )}
            </section>
          )}

          {stack.visible && (
            <section className="d1-modal-section">
              <div className="d1-modal-section-head">
                <h3>{stack.label}</h3>
                <Stepper
                  value={stack.count}
                  min={stack.min}
                  max={stack.max}
                  format={() => stack.value_label}
                  onChange={(value) => onChange({ type: "set_count", value })}
                  ariaLabel={stack.label}
                />
              </div>
              {stack.copy_depth.visible && (
                <FloorLimit
                  control={stack.copy_depth}
                  name="Copies within first"
                  onEnabled={(value) => onChange({ type: "set_copy_depth_enabled", value })}
                  onFloor={(value) => onChange({ type: "set_copy_depth", value })}
                />
              )}
              {stack.count_levels.visible && (
                <>
                  <label className="d1-check">
                    <input
                      type="checkbox"
                      checked={stack.count_levels.enabled}
                      onChange={(event) =>
                        onChange({ type: "set_count_levels", value: event.currentTarget.checked })
                      }
                    />
                    <span>{stack.count_levels.label}</span>
                  </label>
                  {/* This caption explains the switch, so it shows beside it. */}
                  {stack.count_levels.caption_visible && stack.count_levels.caption !== null && (
                    <p className="d1-caption">{stack.count_levels.caption}</p>
                  )}
                  {stack.count_levels.enabled && (
                    <SliderRow
                      label="Levels reach"
                      valueLabel={stack.count_levels.value_label}
                      min={stack.count_levels.min}
                      max={stack.count_levels.max}
                      value={stack.count_levels.value}
                      fill
                      onChange={(value) => onChange({ type: "set_total", value })}
                    />
                  )}
                </>
              )}
            </section>
          )}

          {(effect.visible ||
            form.uncursed.visible ||
            form.source.visible ||
            form.floor_limit.visible) && (
            <section className="d1-modal-section">
              <h3>Details</h3>
              {effect.visible && (
                <>
                  <Field label={effect.label} stack>
                    <Segmented
                      value={effect.mode}
                      options={effect.modes}
                      onChange={(value) => onChange({ type: "set_effect_mode", value })}
                      ariaLabel={`${effect.label} filter`}
                    />
                  </Field>
                  {effect.choices_visible && (
                    <div className="d1-effect-grid" role="group" aria-label="Effects">
                      {effect.groups.flatMap((group) => [
                        <span className="d1-effect-grid-head" key={group.value}>
                          {group.label}
                        </span>,
                        ...effect.choices
                          .filter((choice) => choice.group === group.value)
                          .map((choice) => (
                            <label className="d1-check" key={choice.value}>
                              <input
                                type="checkbox"
                                checked={choice.selected}
                                onChange={() =>
                                  onChange({ type: "toggle_effect", value: choice.value })
                                }
                              />
                              <span>{choice.label}</span>
                            </label>
                          )),
                      ])}
                      <p className="d1-caption d1-effect-grid-note">{effect.caption}</p>
                    </div>
                  )}
                </>
              )}
              {form.uncursed.visible && (
                <CheckBox
                  control={form.uncursed}
                  onChange={(value) => onChange({ type: "set_uncursed", value })}
                />
              )}
              {form.source.visible && (
                <Field label="Source">
                  <select
                    className="d1-select"
                    value={form.source.value ?? ""}
                    onChange={(event) =>
                      onChange({
                        type: "set_source",
                        value: (event.currentTarget.value || null) as ItemSource | null,
                      })
                    }
                  >
                    {form.source.options.map((option) => (
                      <option key={option.value ?? ""} value={option.value ?? ""}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </Field>
              )}
              {form.floor_limit.visible && (
                <FloorLimit
                  control={form.floor_limit}
                  name="Within first"
                  onEnabled={(value) => onChange({ type: "set_floor_limit_enabled", value })}
                  onFloor={(value) => onChange({ type: "set_floor_limit", value })}
                />
              )}
            </section>
          )}

          {resin.include_mage_wand.visible && (
            <section className="d1-modal-section">
              <CheckBox
                control={resin.include_mage_wand}
                onChange={(value) => onChange({ type: "set_include_mage_wand", value })}
              />
            </section>
          )}
          {form.exclude_resin.visible && (
            <section className="d1-modal-section">
              <CheckBox
                control={form.exclude_resin}
                onChange={(value) => onChange({ type: "set_exclude_resin", value })}
              />
            </section>
          )}

          {(form.errors.length > 0 || notice) && (
            <ul className="d1-editor-errors" role="alert">
              {form.errors.map((error) => (
                <li key={error}>{error}</li>
              ))}
              {notice && <li>{notice}</li>}
            </ul>
          )}
        </div>

        <footer className="d1-modal-foot">
          <button type="button" className="d1-btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="d1-btn d1-btn-primary"
            disabled={!form.can_save}
            onClick={onSave}
          >
            {isNew
              ? form.blanket
                ? "Add Blanket Requirement"
                : "Add Requirement"
              : "Save Changes"}
          </button>
        </footer>
      </div>
    </div>
  );
}
