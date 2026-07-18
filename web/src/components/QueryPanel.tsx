import { useState } from 'react'
import { useStore } from '@tanstack/react-store'
import { challenges as challengeOptions, displayItemName, getItem, sourceLabel, wildcardSprites } from '../lib/catalog'
import { floorLabel } from '../lib/region'
import { builtInPresets, loadPresets, queryStore, savePresets, type Preset } from '../lib/store'
import { validateQuery } from '../lib/query'
import type { AnalysisResult, ItemCategory, QueryState, RequirementState } from '../lib/wasm/types'
import { probabilityLabel } from '../lib/format'
import { Challenges } from './Challenges'
import { FloorSlider } from './FloorSlider'
import { ItemSprite } from './ItemSprite'
import { RequirementDialog } from './RequirementDialog'

function requirementTitle(requirement: RequirementState): string {
  if (requirement.item) return displayItemName(requirement.item)
  const base = requirement.kind ? `Any ${requirement.kind}` : 'Any item'
  if (requirement.tier.mode === 'any') return base
  const sign = requirement.tier.mode === 'at_least' ? '+' : requirement.tier.mode === 'at_most' ? ' or lower' : ''
  return `Any tier-${requirement.tier.value}${sign} ${requirement.kind}`
}

function RequirementCard({ requirement, onEdit, onRemove }: { requirement: RequirementState; onEdit: () => void; onRemove: () => void }) {
  const item = requirement.item ? getItem(requirement.item) : undefined
  const category = requirement.kind ?? item?.type
  const badges = [
    requirement.upgrade.mode === 'exact' ? `+${requirement.upgrade.value}` : requirement.upgrade.mode === 'at_least' ? `+${requirement.upgrade.value}+` : undefined,
    requirement.effect,
    requirement.uncursed ? 'uncursed' : undefined,
    requirement.source ? sourceLabel(requirement.source) : undefined,
    requirement.identityGroup ? `group ${String.fromCharCode(64 + requirement.identityGroup)}` : undefined,
    requirement.maxDepth ? `≤ floor ${requirement.maxDepth}` : undefined,
  ].filter(Boolean)
  return (
    <div className="requirement-card" role="button" tabIndex={0} onClick={onEdit} onKeyDown={(event) => { if (event.key === 'Enter' || event.key === ' ') onEdit() }}>
      {category && <ItemSprite spriteIndex={item?.sprite ?? wildcardSprites[category as ItemCategory]} name={item?.name ?? `Any ${category}`} muted={!item} />}
      <div className="requirement-copy"><strong>{requirementTitle(requirement)}</strong><div className="badges">{badges.map((badge) => <span className="badge" key={badge}>{badge}</span>)}</div></div>
      <button type="button" className="remove-button" aria-label={`Remove ${requirementTitle(requirement)}`} onClick={(event) => { event.stopPropagation(); onRemove() }}>×</button>
    </div>
  )
}

export function QueryPanel({ analysis, onStart, onCancel, running }: { analysis?: AnalysisResult; onStart: () => void; onCancel: () => void; running: boolean }) {
  const query = useStore(queryStore, (state) => state)
  const [saved, setSaved] = useState<Preset[]>(loadPresets)
  const [selection, setSelection] = useState('')
  const [dialog, setDialog] = useState<{ index?: number; requirement?: RequirementState }>()
  const validation = validateQuery(query)
  const setQuery = (next: QueryState) => queryStore.setState(() => structuredClone(next))
  const patchQuery = (patch: Partial<QueryState>) => queryStore.setState((state) => ({ ...state, ...patch }))
  const choosePreset = (value: string) => {
    setSelection(value)
    if (!value) return
    const [group, raw] = value.split(':')
    const preset = (group === 'builtin' ? builtInPresets : saved)[Number(raw)]
    if (preset) setQuery(preset.query)
  }
  const savePreset = () => {
    const name = window.prompt('Preset name')?.trim()
    if (!name) return
    const next = [...saved]
    const existing = next.findIndex((preset) => preset.name.toLocaleLowerCase() === name.toLocaleLowerCase())
    const preset = { name, query: structuredClone(query) }
    if (existing >= 0) next[existing] = preset
    else next.push(preset)
    setSaved(next); savePresets(next); setSelection(`saved:${existing >= 0 ? existing : next.length - 1}`)
  }
  const deletePreset = () => {
    if (!selection.startsWith('saved:')) return
    const index = Number(selection.split(':')[1])
    const next = saved.filter((_, candidate) => candidate !== index)
    setSaved(next); savePresets(next); setSelection('')
  }
  const saveRequirement = (requirement: RequirementState) => {
    const requirements = [...query.requirements]
    if (dialog?.index === undefined) requirements.push(requirement)
    else requirements[dialog.index] = requirement
    patchQuery({ requirements }); setDialog(undefined)
  }
  const disabledReason = !validation.valid ? validation.errors[0] : analysis && !analysis.valid ? analysis.error : analysis?.valid && analysis.impossible ? 'No seed can match this query.' : undefined

  return (
    <aside className="query-panel" aria-label="Search query">
      <div className="query-panel-content">
        <div className="preset-row">
          <select aria-label="Preset" value={selection} onChange={(event) => choosePreset(event.currentTarget.value)}>
            <option value="">Choose preset…</option>
            <optgroup label="Built-in">{builtInPresets.map((preset, index) => <option value={`builtin:${index}`} key={preset.name}>{preset.name}</option>)}</optgroup>
            {saved.length > 0 && <optgroup label="Saved">{saved.map((preset, index) => <option value={`saved:${index}`} key={preset.name}>{preset.name}</option>)}</optgroup>}
          </select>
          <button type="button" className="button secondary compact" onClick={savePreset}>Save…</button>
          <button type="button" className="icon-button" aria-label="Delete saved preset" disabled={!selection.startsWith('saved:')} onClick={deletePreset}>⌫</button>
        </div>

        <section className="query-section">
          <div className="section-heading"><h2>Requirements</h2><span>{query.requirements.length}</span></div>
          <div className="requirement-list">
            {query.requirements.map((requirement, index) => <RequirementCard key={index} requirement={requirement} onEdit={() => setDialog({ index, requirement })} onRemove={() => patchQuery({ requirements: query.requirements.filter((_, candidate) => candidate !== index) })} />)}
          </div>
          <button type="button" className="button secondary full" onClick={() => setDialog({})}>+ Add requirement</button>
        </section>

        <section className="query-section">
          <div className="section-heading"><h2>Scope</h2></div>
          <FloorSlider id="query-floor" value={query.maxDepth} onChange={(maxDepth) => patchQuery({ maxDepth })} />
          <div className="checkbox-list">
            <label className="check-row"><input type="checkbox" checked={query.requireBlacksmith} onChange={(event) => patchQuery({ requireBlacksmith: event.currentTarget.checked })} /> Blacksmith reachable</label>
            <label className="check-row"><input type="checkbox" checked={query.excludeBlacksmithRewards} onChange={(event) => patchQuery({ excludeBlacksmithRewards: event.currentTarget.checked })} /> Exclude blacksmith rewards</label>
            <label className="check-row"><input type="checkbox" checked={query.fastMode} onChange={(event) => patchQuery({ fastMode: event.currentTarget.checked })} /> Fast mode</label>
            <p className="field-caption">Skips rare item routes — misses some valid seeds, never returns false ones.</p>
          </div>
        </section>

        <Challenges selected={query.challenges} onChange={(challenges) => patchQuery({ challenges })} />

        {query.requirements.length > 0 && <div className={`feasibility ${analysis && !analysis.valid ? 'error' : analysis?.valid && analysis.impossible ? 'warning' : ''}`} aria-live="polite">
          {!analysis && 'Checking feasibility…'}
          {analysis && !analysis.valid && analysis.error}
          {analysis?.valid && analysis.impossible && <><strong>No seed can match this query</strong>{analysis.notes.map((note) => <span key={note}>{note}</span>)}</>}
          {analysis?.valid && !analysis.impossible && probabilityLabel(analysis.probability)}
        </div>}
      </div>
      <div className="search-action-bar">
        {running
          ? <button className="button danger full" type="button" onClick={onCancel}>Cancel search</button>
          : <button className="button primary full" type="button" onClick={onStart} disabled={Boolean(disabledReason)} title={disabledReason}>Start search</button>}
      </div>
      <RequirementDialog open={Boolean(dialog)} requirement={dialog?.requirement} onClose={() => setDialog(undefined)} onSave={saveRequirement} />
    </aside>
  )
}
