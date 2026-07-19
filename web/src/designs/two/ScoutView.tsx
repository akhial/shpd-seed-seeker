import { useEffect, useMemo, useRef, useState } from 'react'
import { useStore } from '@tanstack/react-store'
import { sourceLabel } from '../../lib/catalog'
import { formatSeedInput } from '../../lib/format'
import { toQueryDocument } from '../../lib/query'
import { regionForDepth, type Region } from '../../lib/region'
import { scoutSeed } from '../../lib/search/coordinator'
import { queryStore } from '../../lib/store'
import type { ScoutItem, ScoutResult } from '../../lib/wasm/types'
import { Sprite } from './Sprite'

export interface ScoutRequestState { seed: string; nonce: number }

const kindTints: Record<ScoutItem['category'], string> = {
  weapon: '#c05e2a', armor: '#3a6ea5', wand: '#7b4fb5', ring: '#a8871f',
}

function accessibilityNote(item: ScoutItem): string | undefined {
  if (item.accessibility.type === 'choice') {
    return `One reward of choice group ${item.accessibility.group} (option ${item.accessibility.option + 1})`
  }
  if (item.accessibility.type === 'scenarios') {
    return `Only in some outcomes of scenario group ${item.accessibility.group}`
  }
  return undefined
}

export function ScoutView({ request }: { request?: ScoutRequestState }) {
  const challenges = useStore(queryStore, (state) => state.challenges)
  const requirementCount = useStore(queryStore, (state) => state.requirements.length)
  const [input, setInput] = useState('')
  const [result, setResult] = useState<ScoutResult | undefined>(undefined)
  const [error, setError] = useState<string | undefined>(undefined)
  const [loading, setLoading] = useState(false)
  const [copied, setCopied] = useState(false)
  const activeScout = useRef(0)

  const run = async (seedCode: string) => {
    const formatted = formatSeedInput(seedCode)
    if (formatted.length !== 11) {
      setError('Seed must use XXX-XXX-XXX format')
      return
    }
    const ticket = ++activeScout.current
    setLoading(true)
    setError(undefined)
    try {
      const query = queryStore.state
      const scouted = await scoutSeed({
        seed: formatted,
        challenges: query.challenges,
        query: query.requirements.length ? toQueryDocument(query) : undefined,
      })
      if (ticket === activeScout.current) setResult(scouted)
    } catch (cause) {
      if (ticket === activeScout.current) {
        setError(cause instanceof Error ? cause.message : String(cause))
        setResult(undefined)
      }
    } finally {
      if (ticket === activeScout.current) setLoading(false)
    }
  }

  useEffect(() => {
    if (!request) return
    const formatted = formatSeedInput(request.seed)
    setInput(formatted)
    void run(formatted)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request])

  const floors = useMemo(() => {
    const byDepth = new Map<number, ScoutItem[]>()
    for (const item of result?.items ?? []) byDepth.set(item.depth, [...(byDepth.get(item.depth) ?? []), item])
    return [...byDepth.entries()].sort(([left], [right]) => left - right)
  }, [result])

  const regionBlocks = useMemo(() => {
    const blocks: { region: Region; floors: [number, ScoutItem[]][] }[] = []
    for (const entry of floors) {
      const region = regionForDepth(entry[0])
      const last = blocks[blocks.length - 1]
      if (last && last.region.name === region.name) last.floors.push(entry)
      else blocks.push({ region, floors: [entry] })
    }
    return blocks
  }, [floors])

  const canScout = input.length === 11 && !loading
  const copySeed = () => {
    if (!result) return
    void navigator.clipboard.writeText(result.seed.code).then(() => {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    })
  }

  return (
    <div className="d2-scout">
      <div className="d2-scout-intro">
        <p className="d2-eyebrow">Dungeon survey</p>
        <h1>Scout a seed</h1>
        <p className="d2-lede">Walk the descent before you take it — every equipment drop, floor by floor through all five regions.</p>
      </div>

      <form
        className="d2-scout-form"
        onSubmit={(event) => { event.preventDefault(); void run(input) }}
      >
        <input
          className="d2-seed-input"
          value={input}
          placeholder="AAA-AAA-AAA"
          autoComplete="off"
          autoCapitalize="characters"
          spellCheck={false}
          aria-label="Seed code"
          onChange={(event) => setInput(formatSeedInput(event.currentTarget.value))}
        />
        <button type="submit" className="d2-btn d2-btn-primary" disabled={!canScout}>
          {loading ? 'Scouting…' : 'Scout'}
        </button>
        {result && (
          <button type="button" className="d2-btn d2-btn-quiet" onClick={copySeed}>
            {copied ? 'Copied ✓' : 'Copy seed'}
          </button>
        )}
      </form>
      {challenges.length > 0 && (
        <p className="d2-scout-note">Simulating with {challenges.length} challenge{challenges.length === 1 ? '' : 's'} enabled.</p>
      )}
      {error && <p className="d2-inline-error" role="alert">{error}</p>}

      {loading && !result && (
        <div className="d2-scout-loading" aria-label="Scouting seed">
          <span /><span /><span />
        </div>
      )}

      {!result && !loading && !error && (
        <div className="d2-scout-empty">
          <p className="d2-results-empty-title">No seed scouted yet</p>
          <p className="d2-results-empty-sub">Enter a canonical seed above, or tap a search result, to inspect its item manifest.</p>
        </div>
      )}

      {result && (
        <section className={`d2-manifest${loading ? ' is-refreshing' : ''}`} aria-label={`Manifest for seed ${result.seed.code}`}>
          <header className="d2-manifest-head">
            <code className="d2-manifest-seed">{result.seed.code}</code>
            <p className="d2-manifest-summary">
              {result.items.length} item{result.items.length === 1 ? '' : 's'} across {floors.length} floor{floors.length === 1 ? '' : 's'}
              {result.totalRequirements > 0 && (
                <>
                  {' · '}
                  <span className={result.matchedRequirements > 0 ? 'd2-summary-match' : undefined}>
                    {result.matchedRequirements} of {result.totalRequirements} requirement{result.totalRequirements === 1 ? '' : 's'} matched
                  </span>
                </>
              )}
              {requirementCount === 0 && ' · add requirements in Find to highlight matches'}
            </p>
          </header>

          <div className="d2-timeline">
            {regionBlocks.map((block) => (
              <section key={block.region.name} className="d2-region" style={{ ['--region' as string]: block.region.color }}>
                <header className="d2-region-head">
                  <span className="d2-region-dot" aria-hidden />
                  <h2>{block.region.name}</h2>
                  <span className="d2-region-range">
                    {block.floors.length === 1
                      ? `floor ${block.floors[0][0]}`
                      : `floors ${block.floors[0][0]}–${block.floors[block.floors.length - 1][0]}`}
                  </span>
                </header>
                {block.floors.map(([depth, items]) => (
                  <div key={depth} className="d2-floor">
                    <h3 className="d2-floor-label">Floor {depth}</h3>
                    <ul className="d2-floor-items">
                      {items.map((item, index) => {
                        const note = accessibilityNote(item)
                        return (
                          <li key={`${item.id}-${index}`} className={`d2-scout-item${item.matched ? ' is-match' : ''}`}>
                            <span className="d2-item-sprite" style={{ ['--tint' as string]: kindTints[item.category] }}>
                              <Sprite index={item.spriteIndex} size={24} />
                            </span>
                            <span className="d2-item-body">
                              <span className="d2-item-title">
                                <span className="d2-item-name">{item.name}</span>
                                {item.upgrade > 0 && <span className="d2-badge d2-badge-up">+{item.upgrade}</span>}
                                {item.cursed && <span className="d2-badge d2-badge-curse">cursed</span>}
                              </span>
                              <span className="d2-item-meta">
                                {item.effect && (
                                  <span className={item.effect.kind === 'curse' ? 'd2-effect-curse' : 'd2-effect-ench'}>
                                    {item.effect.name}{item.effect.kind === 'curse' ? ' · curse' : ''}
                                  </span>
                                )}
                                <span>{sourceLabel(item.source)}</span>
                              </span>
                              {note && <span className="d2-item-access">⑂ {note}</span>}
                            </span>
                            {item.matched && <span className="d2-badge d2-badge-match">✓ Match</span>}
                          </li>
                        )
                      })}
                    </ul>
                  </div>
                ))}
              </section>
            ))}
          </div>
        </section>
      )}
    </div>
  )
}
