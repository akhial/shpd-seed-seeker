import {
  armorCurses,
  armorGlyphs,
  displayItemName,
  getItem,
  isCurseForCategory,
  itemsByCategory,
  sourceLabel,
  sources,
  weaponCurses,
  weaponEnchantments,
  wildcardSprites,
} from '../../lib/catalog'
import { validateRequirement } from '../../lib/query'
import type { ItemCategory, RequirementState, UpgradeFilter } from '../../lib/wasm/types'
import { Field, Segmented, SelectShell, Switch } from './controls'
import { Sprite } from './Sprite'

export const kindLabels: Record<ItemCategory, string> = {
  weapon: 'Weapon', armor: 'Armor', wand: 'Wand', ring: 'Ring',
}

const groupLetters = ['A', 'B', 'C', 'D']

function normalizedUpgrade(upgrade: UpgradeFilter, kind: ItemCategory): UpgradeFilter {
  const maximum = kind === 'ring' ? 4 : 3
  if (upgrade.mode === 'exact') return { mode: 'exact', value: Math.min(Math.max(upgrade.value, 1), maximum) }
  if (upgrade.mode === 'at_least') return { mode: 'at_least', value: Math.min(Math.max(upgrade.value, 1), maximum - 1) }
  return upgrade
}

export function requirementSprite(requirement: RequirementState): number {
  if (requirement.item) return getItem(requirement.item)?.sprite ?? wildcardSprites.weapon
  return wildcardSprites[requirement.kind ?? 'weapon']
}

export function requirementTitle(requirement: RequirementState): string {
  if (requirement.item) return displayItemName(requirement.item)
  const kind = requirement.kind ? kindLabels[requirement.kind].toLowerCase() : 'item'
  return `Any ${kind}`
}

export function requirementChips(requirement: RequirementState): string[] {
  const chips: string[] = []
  if (requirement.tier.mode === 'exact') chips.push(`Tier ${requirement.tier.value}`)
  if (requirement.tier.mode === 'at_least') chips.push(`Tier ${requirement.tier.value}+`)
  if (requirement.tier.mode === 'at_most') chips.push(`Tier ≤ ${requirement.tier.value}`)
  if (requirement.upgrade.mode === 'exact') chips.push(`exactly +${requirement.upgrade.value}`)
  if (requirement.upgrade.mode === 'at_least') chips.push(`+${requirement.upgrade.value} or better`)
  if (requirement.effect) chips.push(requirement.effect)
  if (requirement.uncursed) chips.push('uncursed')
  if (requirement.source) chips.push(sourceLabel(requirement.source))
  if (requirement.identityGroup) chips.push(`Group ${groupLetters[requirement.identityGroup - 1]}`)
  if (requirement.maxDepth !== undefined) chips.push(`first ${requirement.maxDepth} floors`)
  return chips
}

export function RequirementCard({ requirement, expanded, onToggle, onChange, onRemove }: {
  requirement: RequirementState
  expanded: boolean
  onToggle: () => void
  onChange: (requirement: RequirementState) => void
  onRemove: () => void
}) {
  const kind = requirement.kind ?? 'weapon'
  const errors = validateRequirement(requirement)
  const chips = requirementChips(requirement)
  const maxUpgrade = kind === 'ring' ? 4 : 3
  const showTier = !requirement.item && (kind === 'weapon' || kind === 'armor')
  const showEffect = kind === 'weapon' || kind === 'armor'
  const enchantments = kind === 'weapon' ? weaponEnchantments : armorGlyphs
  const curses = kind === 'weapon' ? weaponCurses : armorCurses

  const changeKind = (nextKind: ItemCategory) => onChange({
    ...requirement,
    kind: nextKind,
    item: undefined,
    tier: { mode: 'any', value: 3 },
    effect: undefined,
    upgrade: normalizedUpgrade(requirement.upgrade, nextKind),
  })

  const changeItem = (id: string) => onChange({
    ...requirement,
    item: id || undefined,
    tier: id ? { mode: 'any', value: requirement.tier.value } : requirement.tier,
  })

  const changeTierMode = (mode: RequirementState['tier']['mode']) => {
    let value = requirement.tier.value
    if (mode === 'exact') value = Math.min(Math.max(value, 2), 5)
    if (mode === 'at_least' || mode === 'at_most') value = Math.min(Math.max(value, 3), 4)
    onChange({ ...requirement, tier: { mode, value } })
  }

  const changeUpgradeMode = (mode: UpgradeFilter['mode']) => {
    onChange({ ...requirement, upgrade: normalizedUpgrade({ mode, value: requirement.upgrade.value }, kind) })
  }

  const toggleUncursed = (uncursed: boolean) => onChange({
    ...requirement,
    uncursed,
    effect: uncursed && requirement.effect && isCurseForCategory(kind, requirement.effect) ? undefined : requirement.effect,
  })

  const upgradeValues = requirement.upgrade.mode === 'exact'
    ? Array.from({ length: maxUpgrade }, (_, i) => i + 1)
    : Array.from({ length: maxUpgrade - 1 }, (_, i) => i + 1)

  return (
    <article className={`d2-req d2-kind-${kind}${expanded ? ' is-open' : ''}${errors.length ? ' has-errors' : ''}`}>
      <div className="d2-req-head">
        <button type="button" className="d2-req-summary" onClick={onToggle} aria-expanded={expanded}>
          <span className="d2-req-icon"><Sprite index={requirementSprite(requirement)} size={24} /></span>
          <span className="d2-req-titles">
            <span className="d2-req-title">{requirementTitle(requirement)}</span>
            {chips.length > 0
              ? <span className="d2-req-chips">{chips.map((chip) => <span key={chip} className="d2-chip">{chip}</span>)}</span>
              : <span className="d2-req-chips"><span className="d2-chip d2-chip-quiet">no extra conditions</span></span>}
          </span>
          <span className="d2-req-caret" aria-hidden>{expanded ? '▴' : '▾'}</span>
        </button>
        <button type="button" className="d2-req-remove" aria-label="Remove requirement" title="Remove requirement" onClick={onRemove}>×</button>
      </div>

      {expanded && (
        <div className="d2-req-editor">
          <Field label="Category" wide>
            <Segmented<ItemCategory>
              label="Category"
              value={kind}
              onChange={changeKind}
              options={(['weapon', 'armor', 'wand', 'ring'] as ItemCategory[]).map((value) => ({ value, label: kindLabels[value] }))}
            />
          </Field>

          <Field label="Item" wide>
            <SelectShell>
              <select value={requirement.item ?? ''} onChange={(event) => changeItem(event.currentTarget.value)}>
                <option value="">Any {kindLabels[kind].toLowerCase()}</option>
                {kind === 'weapon'
                  ? [2, 3, 4, 5].map((tier) => (
                      <optgroup key={tier} label={`Tier ${tier}`}>
                        {itemsByCategory.weapon.filter((item) => item.tier === tier).map((item) => (
                          <option key={item.id} value={item.id}>{item.name}</option>
                        ))}
                      </optgroup>
                    ))
                  : itemsByCategory[kind].filter((item) => item.tier !== 1).map((item) => (
                      <option key={item.id} value={item.id}>{item.name}</option>
                    ))}
              </select>
            </SelectShell>
          </Field>

          {showTier && (
            <Field label="Tier" wide>
              <div className="d2-field-stack">
                <Segmented<RequirementState['tier']['mode']>
                  label="Tier predicate"
                  value={requirement.tier.mode}
                  onChange={changeTierMode}
                  options={[
                    { value: 'any', label: 'Any' },
                    { value: 'exact', label: 'Exactly' },
                    { value: 'at_least', label: 'At least' },
                    { value: 'at_most', label: 'At most' },
                  ]}
                />
                {requirement.tier.mode === 'exact' && (
                  <Segmented<number>
                    label="Exact tier"
                    compact
                    value={requirement.tier.value}
                    onChange={(value) => onChange({ ...requirement, tier: { mode: 'exact', value } })}
                    options={[2, 3, 4, 5].map((value) => ({ value, label: `Tier ${value}` }))}
                  />
                )}
                {(requirement.tier.mode === 'at_least' || requirement.tier.mode === 'at_most') && (
                  <Segmented<number>
                    label={requirement.tier.mode === 'at_least' ? 'Minimum tier' : 'Maximum tier'}
                    compact
                    value={requirement.tier.value}
                    onChange={(value) => onChange({ ...requirement, tier: { mode: requirement.tier.mode, value } })}
                    options={[3, 4].map((value) => ({
                      value,
                      label: requirement.tier.mode === 'at_least' ? `Tier ${value} or higher` : `Tier ${value} or lower`,
                    }))}
                  />
                )}
              </div>
            </Field>
          )}

          <Field label="Upgrade level" wide>
            <div className="d2-field-stack">
              <Segmented<UpgradeFilter['mode']>
                label="Upgrade predicate"
                value={requirement.upgrade.mode}
                onChange={changeUpgradeMode}
                options={[
                  { value: 'any', label: 'Any' },
                  { value: 'exact', label: 'Exactly' },
                  { value: 'at_least', label: 'At least' },
                ]}
              />
              {requirement.upgrade.mode !== 'any' && (
                <Segmented<number>
                  label={requirement.upgrade.mode === 'exact' ? 'Exact upgrade' : 'Minimum upgrade'}
                  compact
                  value={requirement.upgrade.value}
                  onChange={(value) => onChange({ ...requirement, upgrade: { mode: requirement.upgrade.mode, value } })}
                  options={upgradeValues.map((value) => ({
                    value,
                    label: requirement.upgrade.mode === 'exact' ? `+${value}` : `+${value} or higher`,
                  }))}
                />
              )}
            </div>
          </Field>

          {showEffect && (
            <Field label={kind === 'weapon' ? 'Enchantment' : 'Glyph'}>
              <SelectShell>
                <select
                  value={requirement.effect ?? ''}
                  onChange={(event) => onChange({ ...requirement, effect: event.currentTarget.value || undefined })}
                >
                  <option value="">None</option>
                  <optgroup label={kind === 'weapon' ? 'Enchantments' : 'Glyphs'}>
                    {enchantments.map((name) => <option key={name} value={name}>{name}</option>)}
                  </optgroup>
                  {!requirement.uncursed && (
                    <optgroup label="Curses">
                      {curses.map((name) => <option key={name} value={name}>{name}</option>)}
                    </optgroup>
                  )}
                </select>
              </SelectShell>
            </Field>
          )}

          <Field label="Source">
            <SelectShell>
              <select
                value={requirement.source ?? ''}
                onChange={(event) => onChange({ ...requirement, source: (event.currentTarget.value || undefined) as RequirementState['source'] })}
              >
                <option value="">Any</option>
                {sources.map((source) => <option key={source.value} value={source.value}>{source.label}</option>)}
              </select>
            </SelectShell>
          </Field>

          <Field label="Same-item group" wide>
            <Segmented<number>
              label="Same-item group"
              value={requirement.identityGroup ?? 0}
              onChange={(value) => onChange({ ...requirement, identityGroup: value || undefined })}
              options={[{ value: 0, label: 'None' }, ...groupLetters.map((letter, index) => ({ value: index + 1, label: letter }))]}
            />
          </Field>

          <div className="d2-field d2-field-wide">
            <Switch
              label="Limit this item to a floor"
              checked={requirement.maxDepth !== undefined}
              onChange={(on) => onChange({ ...requirement, maxDepth: on ? 5 : undefined })}
            />
            {requirement.maxDepth !== undefined && (
              <div className="d2-slider-row">
                <input
                  type="range"
                  min={1}
                  max={24}
                  step={1}
                  value={requirement.maxDepth}
                  aria-label="Requirement floor limit"
                  onChange={(event) => onChange({ ...requirement, maxDepth: Number(event.currentTarget.value) })}
                />
                <span className="d2-slider-value">within first {requirement.maxDepth} floor{requirement.maxDepth === 1 ? '' : 's'}</span>
              </div>
            )}
          </div>

          <div className="d2-field d2-field-wide">
            <Switch label="Require uncursed" checked={requirement.uncursed} onChange={toggleUncursed} />
          </div>

          {errors.length > 0 && (
            <ul className="d2-req-errors d2-field-wide" role="alert">
              {errors.map((error) => <li key={error}>{error}</li>)}
            </ul>
          )}
        </div>
      )}
    </article>
  )
}
