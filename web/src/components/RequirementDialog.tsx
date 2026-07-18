import { useEffect, useMemo, useRef, useState } from 'react'
import {
  armorCurses,
  armorGlyphs,
  getItem,
  itemsByCategory,
  sources,
  weaponCurses,
  weaponEnchantments,
} from '../lib/catalog'
import { emptyRequirement, validateRequirement } from '../lib/query'
import type { ItemCategory, RequirementState, TierFilter, UpgradeFilter } from '../lib/wasm/types'
import { FloorSlider } from './FloorSlider'
import { ItemSprite } from './ItemSprite'

type Props = {
  open: boolean
  requirement?: RequirementState
  onClose: () => void
  onSave: (requirement: RequirementState) => void
}

const categories: { value?: ItemCategory; label: string }[] = [
  { label: 'Any' }, { value: 'weapon', label: 'Weapon' }, { value: 'armor', label: 'Armor' },
  { value: 'wand', label: 'Wand' }, { value: 'ring', label: 'Ring' },
]

export function RequirementDialog({ open, requirement, onClose, onSave }: Props) {
  const dialog = useRef<HTMLDialogElement>(null)
  const [draft, setDraft] = useState<RequirementState>(() => requirement ?? emptyRequirement('weapon'))
  useEffect(() => setDraft(requirement ?? emptyRequirement('weapon')), [requirement, open])
  useEffect(() => {
    if (!dialog.current) return
    if (open && !dialog.current.open && typeof dialog.current.showModal === 'function') dialog.current.showModal()
    if (!open && dialog.current.open) dialog.current.close()
  }, [open])
  const errors = validateRequirement(draft)
  const selectedItem = draft.item ? getItem(draft.item) : undefined
  const tierVisible = (draft.kind === 'weapon' || draft.kind === 'armor') && !draft.item
  const effectVisible = draft.kind === 'weapon' || draft.kind === 'armor'
  const enchantments = draft.kind === 'weapon' ? weaponEnchantments : armorGlyphs
  const curses = draft.kind === 'weapon' ? weaponCurses : armorCurses
  const maxUpgrade = draft.kind === 'ring' ? 4 : 3
  const itemGroups = useMemo(() => {
    if (!draft.kind) return []
    const values = itemsByCategory[draft.kind]
    return [...new Set(values.map((item) => item.tier ?? 0))].map((tier) => ({ tier, values: values.filter((item) => (item.tier ?? 0) === tier) }))
  }, [draft.kind])
  const set = <K extends keyof RequirementState>(key: K, value: RequirementState[K]) => setDraft((current) => ({ ...current, [key]: value }))
  const setKind = (kind?: ItemCategory) => setDraft((current) => ({
    ...current,
    kind,
    item: undefined,
    tier: { mode: 'any', value: 3 },
    effect: kind === 'weapon' || kind === 'armor' ? current.effect : undefined,
    upgrade: current.upgrade.value > (kind === 'ring' ? 4 : 3) ? { mode: 'any', value: 1 } : current.upgrade,
  }))
  const setUncursed = (uncursed: boolean) => setDraft((current) => ({ ...current, uncursed, effect: uncursed && curses.includes(current.effect ?? '') ? undefined : current.effect }))

  return (
    <dialog ref={dialog} className="requirement-dialog" onCancel={(event) => { event.preventDefault(); onClose() }}>
      <form method="dialog" onSubmit={(event) => { event.preventDefault(); if (!errors.length) onSave(draft) }}>
        <div className="dialog-heading">
          <div>
            <p className="eyebrow">Query rule</p>
            <h2>{requirement ? 'Edit requirement' : 'Add requirement'}</h2>
          </div>
          <button type="button" className="icon-button" aria-label="Close dialog" onClick={onClose}>×</button>
        </div>

        <fieldset className="segment-field">
          <legend>Category</legend>
          <div className="segments">
            {categories.map((category) => <button key={category.label} type="button" className={draft.kind === category.value ? 'active' : ''} onClick={() => setKind(category.value)}>{category.label}</button>)}
          </div>
        </fieldset>

        {draft.kind && <label className="field-label">Item
          <span className="input-with-sprite">
            {selectedItem && <ItemSprite spriteIndex={selectedItem.sprite} size={28} name={selectedItem.name} />}
            <select aria-label="Item" value={draft.item ?? ''} onChange={(event) => setDraft((current) => ({ ...current, item: event.currentTarget.value || undefined, tier: { mode: 'any', value: 3 } }))}>
              <option value="">Any {draft.kind}</option>
              {itemGroups.map((group) => <optgroup key={group.tier} label={group.tier ? `Tier ${group.tier}` : draft.kind}>
                {group.values.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
              </optgroup>)}
            </select>
          </span>
        </label>}

        {tierVisible && <div className="paired-fields">
          <label className="field-label">Tier
            <select aria-label="Tier" value={`${draft.tier.mode}:${draft.tier.value}`} onChange={(event) => {
              const [mode, raw] = event.currentTarget.value.split(':') as [TierFilter['mode'], string]
              set('tier', { mode, value: Number(raw) } as TierFilter)
            }}>
              <option value="any:3">Any</option><option value="exact:2">Exactly 2</option><option value="exact:3">Exactly 3</option>
              <option value="exact:4">Exactly 4</option><option value="exact:5">Exactly 5</option>
              <option value="at_least:3">At least 3</option><option value="at_least:4">At least 4</option>
              <option value="at_most:3">At most 3</option><option value="at_most:4">At most 4</option>
            </select>
          </label>
        </div>}

        <div className="paired-fields">
          <label className="field-label">Upgrade
            <select aria-label="Upgrade mode" value={draft.upgrade.mode} onChange={(event) => set('upgrade', { mode: event.currentTarget.value, value: draft.upgrade.value } as UpgradeFilter)}>
              <option value="any">Any</option><option value="exact">Exactly</option><option value="at_least">At least</option>
            </select>
          </label>
          {draft.upgrade.mode !== 'any' && <label className="field-label">Level
            <input aria-label="Upgrade level" type="number" min={draft.upgrade.mode === 'exact' ? 1 : 0} max={maxUpgrade} value={draft.upgrade.value} onChange={(event) => set('upgrade', { ...draft.upgrade, value: event.currentTarget.valueAsNumber })} />
          </label>}
        </div>

        {effectVisible && <label className="field-label">Effect
          <select aria-label="Effect" value={draft.effect ?? ''} onChange={(event) => set('effect', event.currentTarget.value || undefined)}>
            <option value="">None</option>
            <optgroup label={draft.kind === 'weapon' ? 'Enchantments' : 'Glyphs'}>{enchantments.map((effect) => <option key={effect}>{effect}</option>)}</optgroup>
            {!draft.uncursed && <optgroup label="Curses">{curses.map((effect) => <option key={effect}>{effect}</option>)}</optgroup>}
          </select>
        </label>}

        <label className="check-row"><input type="checkbox" checked={draft.uncursed} onChange={(event) => setUncursed(event.currentTarget.checked)} /> Must be uncursed</label>
        <label className="field-label">Source
          <select aria-label="Source" value={draft.source ?? ''} onChange={(event) => set('source', (event.currentTarget.value || undefined) as RequirementState['source'])}>
            <option value="">Any source</option>{sources.map((source) => <option value={source.value} key={source.value}>{source.label}</option>)}
          </select>
        </label>
        <label className="field-label">Identity group
          <select aria-label="Identity group" value={draft.identityGroup ?? ''} onChange={(event) => set('identityGroup', event.currentTarget.value ? Number(event.currentTarget.value) : undefined)}>
            <option value="">None</option>{['A', 'B', 'C', 'D'].map((letter, index) => <option value={index + 1} key={letter}>{letter}</option>)}
          </select>
          <small>Requirements in the same group must be the same item kind (unidentified scroll/potion logic).</small>
        </label>
        <label className="check-row"><input type="checkbox" checked={draft.maxDepth !== undefined} onChange={(event) => set('maxDepth', event.currentTarget.checked ? 12 : undefined)} /> Limit floor</label>
        {draft.maxDepth !== undefined && <FloorSlider id="requirement-floor" value={draft.maxDepth} onChange={(value) => set('maxDepth', value)} />}
        {errors.length > 0 && <div className="inline-error" role="alert">{errors.join(' ')}</div>}
        <div className="dialog-actions">
          <button type="button" className="button secondary" onClick={onClose}>Cancel</button>
          <button type="submit" className="button primary" disabled={errors.length > 0}>{requirement ? 'Save' : 'Add'}</button>
        </div>
      </form>
    </dialog>
  )
}
