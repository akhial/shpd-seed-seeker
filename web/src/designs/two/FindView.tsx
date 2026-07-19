import { useEffect, useMemo, useRef, useState } from 'react'
import { useStore } from '@tanstack/react-store'
import { challenges as challengeOptions, wildcardSprites } from '../../lib/catalog'
import { compactNumber, formatDuration, probabilityLabel } from '../../lib/format'
import { emptyRequirement, toQueryDocument, toQueryJson, validateQuery } from '../../lib/query'
import { regionForDepth } from '../../lib/region'
import { SearchCoordinator, searchStore } from '../../lib/search/coordinator'
import { builtInPresets, loadPresets, savePresets, queryStore, type Preset } from '../../lib/store'
import { fromQueryJson } from '../../lib/query'
import { analyzeQuery } from '../../lib/wasm'
import type { AnalysisResult, ChallengeName, ItemCategory, ParsedSeed, QueryState, RequirementState } from '../../lib/wasm/types'
import { Switch } from './controls'
import { useDebounced, useEngineInfo } from './hooks'
import { kindLabels, RequirementCard } from './RequirementCard'
import { Sprite } from './Sprite'

const REGION_STOPS = [1, 6, 11, 16, 21]
const generationChallenges = new Set<ChallengeName>(['barren_land', 'into_darkness', 'forbidden_runes'])

function etaLabel(probability: number | null | undefined, rate: number): string {
  if (!probability || probability <= 0 || rate <= 0) return '—'
  const seconds = 1 / (probability * rate)
  if (seconds < 1) return 'under a second'
  if (seconds < 90) return `≈ ${Math.round(seconds)}s`
  if (seconds < 90 * 60) return `≈ ${Math.round(seconds / 60)}m`
  if (seconds < 36 * 3600) return `≈ ${Math.round(seconds / 3600)}h`
  return `≈ ${Math.round(seconds / 86400)}d`
}

function setQuery(patch: Partial<QueryState>): void {
  queryStore.setState((state) => ({ ...state, ...patch }))
}

export function FindView({ onScout }: { onScout: (seed: string) => void }) {
  const query = useStore(queryStore, (state) => state)
  const search = useStore(searchStore, (state) => state)
  const engine = useEngineInfo()
  const coordinator = useRef<SearchCoordinator | undefined>(undefined)
  const [expanded, setExpanded] = useState<number | null>(null)
  const [userPresets, setUserPresets] = useState<Preset[]>(() => loadPresets())
  const [savingPreset, setSavingPreset] = useState(false)
  const [presetName, setPresetName] = useState('')
  const [copiedSeed, setCopiedSeed] = useState<string | null>(null)
  const [analysis, setAnalysis] = useState<AnalysisResult | undefined>(undefined)
  const [challengesOpen, setChallengesOpen] = useState(() => queryStore.state.challenges.length > 0)

  useEffect(() => {
    if (engine && !coordinator.current) coordinator.current = new SearchCoordinator(engine.totalSeeds)
  }, [engine])

  const serialized = toQueryJson(query)
  const debouncedJson = useDebounced(serialized, 250)
  const hasRequirements = query.requirements.length > 0
  useEffect(() => {
    if (!hasRequirements) { setAnalysis(undefined); return }
    let alive = true
    void analyzeQuery(debouncedJson).then((result) => { if (alive) setAnalysis(result) }).catch(() => undefined)
    return () => { alive = false }
  }, [debouncedJson, hasRequirements])

  const validation = useMemo(() => validateQuery(query), [query])
  const queryErrors = validation.errors.filter((error) => !error.startsWith('Requirement ') && error !== 'Add at least one requirement.')
  const impossible = analysis?.valid === true && analysis.impossible
  const probability = analysis?.valid === true ? analysis.probability : null
  const notes = analysis?.valid === true ? analysis.notes : []
  const running = search.state === 'running'
  const canStart = hasRequirements && validation.valid && !!engine && !impossible

  const setRequirement = (index: number, requirement: RequirementState) =>
    setQuery({ requirements: query.requirements.map((entry, i) => (i === index ? requirement : entry)) })
  const removeRequirement = (index: number) => {
    setExpanded(null)
    setQuery({ requirements: query.requirements.filter((_, i) => i !== index) })
  }
  const addRequirement = (kind: ItemCategory) => {
    setQuery({ requirements: [...query.requirements, emptyRequirement(kind)] })
    setExpanded(query.requirements.length)
  }

  const applyPreset = (preset: Preset) => {
    setExpanded(null)
    queryStore.setState(() => fromQueryJson(toQueryJson(preset.query)))
  }
  const saveCurrentPreset = () => {
    const name = presetName.trim()
    if (!name) return
    const existing = userPresets.findIndex((preset) => preset.name.toLowerCase() === name.toLowerCase())
    const entry: Preset = { name, query: fromQueryJson(toQueryJson(query)) }
    const next = existing >= 0
      ? userPresets.map((preset, index) => (index === existing ? entry : preset))
      : [...userPresets, entry]
    setUserPresets(next)
    savePresets(next)
    setSavingPreset(false)
    setPresetName('')
  }
  const deletePreset = (target: Preset) => {
    const next = userPresets.filter((preset) => preset !== target)
    setUserPresets(next)
    savePresets(next)
  }

  const toggleChallenge = (name: ChallengeName, on: boolean) =>
    setQuery({ challenges: on ? [...query.challenges, name] : query.challenges.filter((value) => value !== name) })

  const copySeed = (seed: string) => {
    void navigator.clipboard.writeText(seed).then(() => {
      setCopiedSeed(seed)
      window.setTimeout(() => setCopiedSeed((current) => (current === seed ? null : current)), 1500)
    })
  }

  const progress = search.total > 0 ? Math.min(100, (search.tested / search.total) * 100) : 0
  const scopeRegion = regionForDepth(query.maxDepth)

  return (
    <div className="d2-find">
      <section className="d2-plan" aria-label="Plan the expedition">
        <div className="d2-plan-intro">
          <p className="d2-eyebrow">Plan the expedition</p>
          <h1>What should this run drop?</h1>
          <p className="d2-lede">Describe the gear you want, choose how deep to look, and Seed Seeker will comb every dungeon seed for a run that delivers.</p>
        </div>

        <div className="d2-presets-bar">
          <span className="d2-select d2-grow">
            <select
              value=""
              aria-label="Load a preset"
              onChange={(event) => {
                const [scope, index] = event.currentTarget.value.split(':')
                const list = scope === 'b' ? builtInPresets : userPresets
                const preset = list[Number(index)]
                if (preset) applyPreset(preset)
              }}
            >
              <option value="" disabled>Load a preset…</option>
              <optgroup label="Included">
                {builtInPresets.map((preset, index) => <option key={preset.name} value={`b:${index}`}>{preset.name}</option>)}
              </optgroup>
              {userPresets.length > 0 && (
                <optgroup label="Saved">
                  {userPresets.map((preset, index) => <option key={preset.name} value={`u:${index}`}>{preset.name}</option>)}
                </optgroup>
              )}
            </select>
          </span>
          {savingPreset ? (
            <form
              className="d2-preset-save"
              onSubmit={(event) => { event.preventDefault(); saveCurrentPreset() }}
            >
              <input
                autoFocus
                value={presetName}
                placeholder="Preset name"
                aria-label="Preset name"
                onChange={(event) => setPresetName(event.currentTarget.value)}
                onKeyDown={(event) => { if (event.key === 'Escape') setSavingPreset(false) }}
              />
              <button type="submit" className="d2-btn d2-btn-small" disabled={!presetName.trim()}>Save</button>
              <button type="button" className="d2-btn d2-btn-small d2-btn-quiet" onClick={() => setSavingPreset(false)}>Cancel</button>
            </form>
          ) : (
            <button type="button" className="d2-btn d2-btn-small" onClick={() => { setPresetName(''); setSavingPreset(true) }}>
              Save current
            </button>
          )}
        </div>
        {userPresets.length > 0 && (
          <ul className="d2-preset-chips" aria-label="Saved presets">
            {userPresets.map((preset) => (
              <li key={preset.name}>
                <button type="button" className="d2-preset-chip-apply" onClick={() => applyPreset(preset)}>{preset.name}</button>
                <button type="button" className="d2-preset-chip-delete" aria-label={`Delete preset ${preset.name}`} title="Delete preset" onClick={() => deletePreset(preset)}>×</button>
              </li>
            ))}
          </ul>
        )}

        <section className="d2-step">
          <header className="d2-step-head">
            <span className="d2-step-num">1</span>
            <h2>Target items</h2>
            {hasRequirements && <span className="d2-count-badge">{query.requirements.length}</span>}
          </header>
          {query.requirements.length === 0 && (
            <p className="d2-empty-hint">No requirements yet. Add one to describe the item you're hunting for.</p>
          )}
          <div className="d2-req-list">
            {query.requirements.map((requirement, index) => (
              <RequirementCard
                key={index}
                requirement={requirement}
                expanded={expanded === index}
                onToggle={() => setExpanded(expanded === index ? null : index)}
                onChange={(next) => setRequirement(index, next)}
                onRemove={() => removeRequirement(index)}
              />
            ))}
          </div>
          <div className="d2-add-row" role="group" aria-label="Add a requirement">
            {(['weapon', 'armor', 'wand', 'ring'] as ItemCategory[]).map((kind) => (
              <button key={kind} type="button" className={`d2-add-kind d2-kind-${kind}`} onClick={() => addRequirement(kind)}>
                <Sprite index={wildcardSprites[kind]} size={20} />
                <span>{kindLabels[kind]}</span>
                <span className="d2-add-plus" aria-hidden>+</span>
              </button>
            ))}
          </div>
        </section>

        <section className="d2-step">
          <header className="d2-step-head">
            <span className="d2-step-num">2</span>
            <h2>Search scope</h2>
          </header>
          <div className="d2-scope">
            <div className="d2-scope-label-row">
              <span className="d2-field-label">Floor limit</span>
              <span className="d2-scope-value">
                first {query.maxDepth} floor{query.maxDepth === 1 ? '' : 's'}
                <span className="d2-region-tag" style={{ ['--region' as string]: scopeRegion.color }}>{scopeRegion.name}</span>
              </span>
            </div>
            <input
              type="range"
              min={1}
              max={24}
              step={1}
              value={query.maxDepth}
              aria-label="Floor limit"
              onChange={(event) => setQuery({ maxDepth: Number(event.currentTarget.value) })}
            />
            <div className="d2-region-strip" aria-hidden>
              {REGION_STOPS.map((start) => {
                const region = regionForDepth(start)
                return <span key={region.name} style={{ background: region.color }} title={region.name} />
              })}
              <span className="d2-region-marker" style={{ left: `${((query.maxDepth - 1) / 23) * 100}%` }} />
            </div>
            <div className="d2-region-legend" aria-hidden>
              <span>Sewers</span><span>Prison</span><span>Caves</span><span>City</span><span>Halls</span>
            </div>
          </div>
          <div className="d2-scope-toggles">
            <Switch
              label="Require accessible blacksmith"
              caption={query.maxDepth >= 14 ? 'The Smith is always reachable when searching 14+ floors.' : 'Only keep seeds where the Smith quest can be reached in scope.'}
              checked={query.requireBlacksmith}
              disabled={query.maxDepth >= 14}
              onChange={(on) => setQuery({ requireBlacksmith: on })}
            />
            <Switch
              label="Exclude Smith rewards"
              caption="Required items cannot come from the 2,000-favor Smith choice, leaving favor available for reforging."
              checked={query.excludeBlacksmithRewards}
              onChange={(on) => setQuery({ excludeBlacksmithRewards: on })}
            />
            <Switch
              label="Fast search"
              caption="Treats +3 weapons and armor as quest rewards only, skipping the rare Crypt and Sacrificial-fire prizes. Found seeds are always genuine."
              checked={query.fastMode}
              onChange={(on) => setQuery({ fastMode: on })}
            />
          </div>
        </section>

        <section className="d2-step">
          <details
            className="d2-details"
            open={challengesOpen}
            onToggle={(event) => setChallengesOpen(event.currentTarget.open)}
          >
            <summary className="d2-step-head d2-summary">
              <span className="d2-step-num">3</span>
              <h2>Challenges</h2>
              {query.challenges.length > 0 && <span className="d2-count-badge d2-count-badge-accent">{query.challenges.length} on</span>}
              <span className="d2-details-caret" aria-hidden>▾</span>
            </summary>
            <p className="d2-empty-hint">Searches simulate runs with the selected challenges enabled.</p>
            <div className="d2-challenge-list">
              {challengeOptions.map((challenge) => (
                <Switch
                  key={challenge.value}
                  label={challenge.label}
                  caption={generationChallenges.has(challenge.value) ? 'changes level generation' : 'no effect on seed content'}
                  checked={query.challenges.includes(challenge.value)}
                  onChange={(on) => toggleChallenge(challenge.value, on)}
                />
              ))}
            </div>
          </details>
        </section>
      </section>

      <section className="d2-launch" aria-label="Search">
        <div className="d2-launch-card">
          <div className="d2-launch-head">
            <div>
              <p className="d2-eyebrow">Expedition</p>
              <h2>Launch the search</h2>
            </div>
            <p className="d2-launch-scale">
              {engine ? 'Runs entirely in your browser' : 'Warming up the engine…'}
            </p>
          </div>

          {hasRequirements && analysis?.valid === true && !impossible && (
            <p className="d2-analysis">{probabilityLabel(probability)}{query.challenges.length > 0 ? ` · simulating ${query.challenges.length} challenge${query.challenges.length === 1 ? '' : 's'}` : ''}</p>
          )}
          {notes.map((note) => <p key={note} className="d2-analysis-note">{note}</p>)}

          {impossible && (
            <div className="d2-banner d2-banner-warn" role="alert">
              <strong>Impossible query</strong>
              <p>No seed can satisfy these requirements within the current floor limit. Quest-reward-only items need their quest floors in range: +3 wands floor 9, +3/+4 rings floor 19.</p>
            </div>
          )}
          {queryErrors.length > 0 && (
            <ul className="d2-banner d2-banner-error" role="alert">
              {queryErrors.map((error) => <li key={error}>{error}</li>)}
            </ul>
          )}
          {search.error && (
            <div className="d2-banner d2-banner-error" role="alert"><p>Search failed: {search.error}</p></div>
          )}

          <button
            type="button"
            className={`d2-start${running ? ' is-running' : ''}`}
            disabled={!running && !canStart}
            onClick={() => {
              if (running) coordinator.current?.cancel()
              else coordinator.current?.start(toQueryDocument(queryStore.state))
            }}
          >
            {running ? 'Cancel search' : 'Start search'}
          </button>

          {running && (
            <div className="d2-progress" role="status" aria-label="Search progress">
              <div className="d2-progress-track"><div className="d2-progress-fill" style={{ width: `${progress}%` }} /></div>
              <dl className="d2-stats">
                <div><dt>Tested</dt><dd>{compactNumber(search.tested)}</dd></div>
                <div><dt>Rate</dt><dd>{compactNumber(search.rate)}<span className="d2-stat-sub">/s</span></dd></div>
                <div><dt>Elapsed</dt><dd>{formatDuration(search.elapsed)}</dd></div>
                <div><dt>First seed</dt><dd>{etaLabel(probability, search.rate)}</dd></div>
              </dl>
            </div>
          )}

          {!running && search.state === 'completed' && (
            <p className="d2-status-line d2-status-done">
              Search complete — {search.matches.length ? `${search.matches.length.toLocaleString()} seed${search.matches.length === 1 ? '' : 's'} found` : 'no matching seeds'} in {formatDuration(search.elapsed)}.
            </p>
          )}
          {!running && search.state === 'cancelled' && !search.error && (
            <p className="d2-status-line">Search cancelled — {search.matches.length.toLocaleString()} seed{search.matches.length === 1 ? '' : 's'} kept.</p>
          )}
          {search.capped && <p className="d2-status-line d2-status-capped">Result limit reached (1,024 seeds).</p>}
        </div>

        <ResultsGrid search={{ matches: search.matches, state: search.state }} copiedSeed={copiedSeed} onCopy={copySeed} onScout={onScout} />
      </section>
    </div>
  )
}

function ResultsGrid({ search, copiedSeed, onCopy, onScout }: {
  search: { matches: ParsedSeed[]; state: string }
  copiedSeed: string | null
  onCopy: (seed: string) => void
  onScout: (seed: string) => void
}) {
  if (search.matches.length === 0) {
    return (
      <div className="d2-results-empty">
        <div className="d2-results-empty-sprites">
          <Sprite index={wildcardSprites.wand} size={28} />
          <Sprite index={wildcardSprites.armor} size={28} />
          <Sprite index={wildcardSprites.ring} size={28} />
        </div>
        <p className="d2-results-empty-title">
          {search.state === 'running' ? 'Scanning the dungeon…' : search.state === 'idle' ? 'Your matching seeds will land here' : 'No seeds matched'}
        </p>
        <p className="d2-results-empty-sub">
          {search.state === 'running'
            ? 'Matches appear the moment a worker finds one.'
            : search.state === 'idle'
              ? 'Plan your expedition on the left, then launch the search.'
              : 'Try widening the floor limit or relaxing a requirement.'}
        </p>
      </div>
    )
  }
  return (
    <div className="d2-results">
      <header className="d2-results-head">
        <h3>Matching seeds</h3>
        <span className="d2-count-badge">{search.matches.length.toLocaleString()}</span>
      </header>
      <ul className="d2-seed-grid">
        {search.matches.map((seed, index) => (
          <li key={seed.code} className="d2-seed-card">
            <div className="d2-seed-top">
              <span className="d2-seed-idx">#{index + 1}</span>
              <code className="d2-seed-code">{seed.code}</code>
            </div>
            <div className="d2-seed-actions">
              <button type="button" className="d2-btn d2-btn-small d2-btn-quiet" onClick={() => onCopy(seed.code)}>
                {copiedSeed === seed.code ? 'Copied ✓' : 'Copy'}
              </button>
              <button type="button" className="d2-btn d2-btn-small" onClick={() => onScout(seed.code)}>Scout →</button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  )
}
