import { useEffect, useMemo, useRef, useState } from 'react'
import { createRootRoute, createRoute, createRouter, Link, Outlet, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { useStore } from '@tanstack/react-store'
import { Challenges } from './components/Challenges'
import { ItemSprite } from './components/ItemSprite'
import { QueryPanel } from './components/QueryPanel'
import { SearchResults } from './components/SearchResults'
import { challenges as challengeOptions, displayItemName, sourceLabel } from './lib/catalog'
import { formatSeedInput } from './lib/format'
import { regionForDepth } from './lib/region'
import { toQueryDocument, toQueryJson } from './lib/query'
import { SearchCoordinator, scoutSeed, searchStore } from './lib/search/coordinator'
import { queryStore } from './lib/store'
import { analyzeQuery, formatSeedCode, getEngineInfo, parseSeedCode } from './lib/wasm'
import type { ChallengeName, ScoutItem } from './lib/wasm/types'

function usePageTitle(title: string) {
  useEffect(() => { document.title = title }, [title])
}

function useDebounced<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => { const timer = window.setTimeout(() => setDebounced(value), delay); return () => window.clearTimeout(timer) }, [value, delay])
  return debounced
}

function RootLayout() {
  const engine = useQuery({ queryKey: ['engine-info'], queryFn: getEngineInfo, staleTime: Infinity })
  return <div className="app-shell">
    <header className="site-header">
      <Link to="/" className="wordmark"><ItemSprite spriteIndex={112} size={16} name="Sword" /><span>Seed Seeker</span></Link>
      <nav aria-label="Primary navigation">
        <Link to="/" activeOptions={{ exact: true }} activeProps={{ className: 'active' }}>Find</Link>
        <Link to="/scout" activeProps={{ className: 'active' }}>Scout</Link>
        <Link to="/about" activeProps={{ className: 'active' }}>About</Link>
      </nav>
    </header>
    <div className="route-content"><Outlet /></div>
    <footer className="site-footer">
      Unofficial companion for Shattered Pixel Dungeon v{engine.data?.shpdVersion ?? '…'} · not endorsed by its developers · GPL-3.0-or-later · <a href="https://github.com/akhial/shpd-seed-seeker">Source</a> · <Link to="/about">Licenses</Link>
    </footer>
  </div>
}

function FinderPage() {
  usePageTitle('Find seeds — Seed Seeker')
  const navigate = useNavigate()
  const query = useStore(queryStore, (state) => state)
  const search = useStore(searchStore, (state) => state)
  const engine = useQuery({ queryKey: ['engine-info'], queryFn: getEngineInfo, staleTime: Infinity })
  const coordinator = useRef<SearchCoordinator | undefined>(undefined)
  useEffect(() => { if (engine.data && !coordinator.current) coordinator.current = new SearchCoordinator(engine.data.totalSeeds) }, [engine.data])
  const serialized = toQueryJson(query)
  const debounced = useDebounced(serialized, 250)
  const analysis = useQuery({ queryKey: ['analyze', debounced], queryFn: () => analyzeQuery(debounced), enabled: query.requirements.length > 0 })
  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => { if (search.state === 'running') event.preventDefault() }
    window.addEventListener('beforeunload', warn)
    return () => window.removeEventListener('beforeunload', warn)
  }, [search.state])
  return <div className="finder-layout">
    <QueryPanel analysis={analysis.data} running={search.state === 'running'} onStart={() => coordinator.current?.start(toQueryDocument(query))} onCancel={() => coordinator.current?.cancel()} />
    <SearchResults search={search} onScout={(seed) => void navigate({ to: '/scout', search: { seed } })} />
  </div>
}

function challengeFromSearch(raw?: string): ChallengeName[] {
  const valid = new Set(challengeOptions.map((challenge) => challenge.value))
  return (raw?.split(',') ?? []).filter((value): value is ChallengeName => valid.has(value as ChallengeName))
}

export function SeedInput({ value, onChange, onScout, busy = false }: { value: string; onChange: (value: string) => void; onScout: () => void; busy?: boolean }) {
  const formatted = formatSeedInput(value)
  return <div className="seed-input-row">
    <input aria-label="Seed code" className="seed-input" value={formatted} placeholder="AAA-AAA-AAA" autoComplete="off" spellCheck={false} onChange={(event) => {
      const raw = event.currentTarget.value
      const immediate = formatSeedInput(raw)
      onChange(immediate)
      void formatSeedCode(raw).then(onChange).catch(() => undefined)
    }} onKeyDown={(event) => { if (event.key === 'Enter' && formatted.length === 11) onScout() }} />
    <button type="button" className="button primary" disabled={formatted.length !== 11 || busy} onClick={onScout}>{busy ? 'Scouting…' : 'Scout'}</button>
  </div>
}

const groupLetter = (group: number) => String.fromCharCode(65 + (group % 26))

function accessibilityLabel(item: ScoutItem): string | undefined {
  if (item.accessibility.type === 'choice') return `Choice ${groupLetter(item.accessibility.group)}·${item.accessibility.option + 1}`
  if (item.accessibility.type === 'scenarios') return `Conditional ${groupLetter(item.accessibility.group)}`
  return undefined
}

function ScoutPage() {
  const search = scoutRoute.useSearch()
  const navigate = useNavigate({ from: '/scout' })
  const finderQuery = useStore(queryStore, (state) => state)
  const [seedInput, setSeedInput] = useState(search.seed ?? '')
  const [requestedSeed, setRequestedSeed] = useState(search.seed ?? '')
  const [selectedChallenges, setSelectedChallenges] = useState<ChallengeName[]>(() => challengeFromSearch(search.challenges))
  const [highlight, setHighlight] = useState(finderQuery.requirements.length > 0)
  const [inputError, setInputError] = useState<string | undefined>(() => search.invalidSeed ? 'Seed codes must contain exactly nine letters.' : undefined)
  const queryJson = highlight && finderQuery.requirements.length ? toQueryJson(finderQuery) : undefined
  usePageTitle(requestedSeed ? `Scout ${requestedSeed} — Seed Seeker` : 'Scout a seed — Seed Seeker')
  const result = useQuery({
    queryKey: ['scout', requestedSeed, selectedChallenges.join(','), queryJson],
    queryFn: async () => {
      const parsed = await parseSeedCode(requestedSeed)
      return scoutSeed({ seed: parsed.code, challenges: selectedChallenges, query: queryJson ? toQueryDocument(finderQuery) : undefined })
    },
    enabled: requestedSeed.length === 11,
    retry: false,
  })
  const submit = async () => {
    try {
      const parsed = await parseSeedCode(seedInput)
      setInputError(undefined); setSeedInput(parsed.code); setRequestedSeed(parsed.code)
      void navigate({ search: { seed: parsed.code, challenges: selectedChallenges.length ? selectedChallenges.join(',') : undefined }, replace: true })
    } catch (error) {
      setInputError(error instanceof Error ? error.message : String(error))
    }
  }
  const updateChallenges = (challenges: ChallengeName[]) => {
    setSelectedChallenges(challenges)
    if (requestedSeed) void navigate({ search: { seed: requestedSeed, challenges: challenges.length ? challenges.join(',') : undefined }, replace: true })
  }
  const copyLink = async () => {
    const url = new URL(window.location.href)
    if (requestedSeed || seedInput.length === 11) url.searchParams.set('seed', requestedSeed || seedInput)
    else url.searchParams.delete('seed')
    if (selectedChallenges.length) url.searchParams.set('challenges', selectedChallenges.join(','))
    else url.searchParams.delete('challenges')
    await navigator.clipboard.writeText(url.toString())
  }
  const groups = useMemo(() => {
    const map = new Map<number, ScoutItem[]>()
    for (const item of result.data?.items ?? []) map.set(item.depth, [...(map.get(item.depth) ?? []), item])
    return [...map.entries()].sort(([left], [right]) => left - right)
  }, [result.data])
  const floorCount = new Set(result.data?.items.map((item) => item.depth)).size
  return <main className="scout-page page-column">
    <div className="page-intro"><p className="eyebrow">Dungeon survey</p><h1>Scout a seed</h1><p>See every generated equipment item, floor by floor.</p></div>
    <SeedInput value={seedInput} onChange={setSeedInput} onScout={() => void submit()} busy={result.isFetching} />
    {inputError && <p className="inline-error" role="alert">{inputError}</p>}
    <div className="scout-controls">
      <Challenges selected={selectedChallenges} onChange={updateChallenges} />
      <label className="check-row"><input type="checkbox" checked={highlight} disabled={!finderQuery.requirements.length} onChange={(event) => setHighlight(event.currentTarget.checked)} /> Highlight current query</label>
      <button type="button" className="button secondary compact" onClick={() => void copyLink()}>Copy link</button>
    </div>
    {result.isLoading && <div className="skeleton-card" aria-label="Scouting seed"><span /><span /><span /><span /></div>}
    {result.error && <div className="banner error">{result.error.message}</div>}
    {result.data && <>
      <section className="scout-summary">
        <div><p className="seed-code large">{result.data.seed.code}</p><p>{result.data.items.length} items across {floorCount} floors</p></div>
        {result.data.totalRequirements > 0 && <div className={result.data.matchedRequirements === result.data.totalRequirements ? 'match-complete' : 'muted'}>Matches {result.data.matchedRequirements} of {result.data.totalRequirements} requirements</div>}
      </section>
      <div className="floor-groups">{groups.map(([depth, items]) => {
        const region = regionForDepth(depth)
        return <section className="floor-group" key={depth}>
          <h2><span className="region-dot" style={{ background: region.color }} />Floor {depth} · {region.name}</h2>
          <div className="item-list">{items.map((item, index) => <div className={`scout-item ${item.matched ? 'matched' : ''}`} key={`${item.id}-${index}`}>
            <ItemSprite spriteIndex={item.spriteIndex} name={displayItemName(item.id)} />
            <div className="scout-item-name"><strong>{displayItemName(item.id)} {item.upgrade > 0 && <span className="upgrade">+{item.upgrade}</span>}</strong><div className="badges">
              {item.effect && <span className={`badge ${item.effect.kind === 'curse' ? 'curse' : 'effect'}`}>{item.effect.name}</span>}
              {item.cursed && item.effect?.kind !== 'curse' && <span className="badge curse">cursed</span>}
              <span className="badge subtle">{sourceLabel(item.source)}</span>
              {accessibilityLabel(item) && <span className="badge choice">{accessibilityLabel(item)}</span>}
            </div></div>
          </div>)}</div>
        </section>
      })}</div>
    </>}
  </main>
}

function AboutPage() {
  usePageTitle('About — Seed Seeker')
  const engine = useQuery({ queryKey: ['engine-info'], queryFn: getEngineInfo, staleTime: Infinity })
  return <main className="about-page page-column">
    <div className="page-intro"><p className="eyebrow">About the project</p><h1>Built to search the whole dungeon.</h1></div>
    <section className="about-card"><h2>What it is</h2><p>Seed Seeker is an extremely fast seed finder for Shattered Pixel Dungeon. It runs entirely in your browser through WebAssembly and Web Workers—your queries and seeds are never uploaded.</p></section>
    <section className="about-card"><h2>Unofficial companion</h2><p>This project is not endorsed by the Shattered Pixel Dungeon developers. The engine targets upstream v{engine.data?.shpdVersion ?? '3.3.8'} at commit <code>{engine.data?.shpdCommit?.slice(0, 12) ?? '…'}</code>.</p></section>
    <section className="about-card"><h2>Attribution</h2><p>Pixel Dungeon © 2012–2015 Oleg Dolya. Shattered Pixel Dungeon © 2014–2026 Evan Debenham. The sprites here are unchanged copies from v3.3.8.</p><a href="https://github.com/00-Evan/shattered-pixel-dungeon">Shattered Pixel Dungeon on GitHub</a></section>
    <section className="about-card"><h2>License</h2><p>Seed Seeker is GPL-3.0-or-later.</p><div className="link-list"><a href="/licenses/COPYING.txt">GPL license</a><a href="/licenses/NOTICE.txt">Notice</a><a href="/third_party/shattered-pixel-dungeon/ATTRIBUTION.md">Sprite attribution</a><a href="https://github.com/akhial/shpd-seed-seeker">Source repository</a></div></section>
    <section className="about-card"><h2>Other apps</h2><p>Prefer a native build? <a href="https://github.com/akhial/shpd-seed-seeker/releases">Download Seed Seeker for Android, macOS, Linux, or Windows.</a></p></section>
  </main>
}

const rootRoute = createRootRoute({ component: RootLayout })
const finderRoute = createRoute({ getParentRoute: () => rootRoute, path: '/', component: FinderPage })
export const scoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/scout',
  validateSearch: (search: Record<string, unknown>): { seed?: string; challenges?: string; invalidSeed?: boolean } => {
    const rawSeed = typeof search.seed === 'string' ? search.seed : undefined
    const seed = rawSeed === undefined ? undefined : formatSeedInput(rawSeed)
    return {
      seed,
      challenges: typeof search.challenges === 'string' ? search.challenges : undefined,
      invalidSeed: rawSeed !== undefined && seed?.length !== 11,
    }
  },
  component: ScoutPage,
})
const aboutRoute = createRoute({ getParentRoute: () => rootRoute, path: '/about', component: AboutPage })
const routeTree = rootRoute.addChildren([finderRoute, scoutRoute, aboutRoute])
export const router = createRouter({ routeTree, defaultPreload: 'intent' })

declare module '@tanstack/react-router' {
  interface Register { router: typeof router }
}
