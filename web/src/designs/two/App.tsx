import { useEffect, useState } from 'react'
import { useStore } from '@tanstack/react-store'
import { wildcardSprites } from '../../lib/catalog'
import { searchStore } from '../../lib/search/coordinator'
import { FindView } from './FindView'
import { ScoutView, type ScoutRequestState } from './ScoutView'
import { useEngineInfo } from './hooks'
import { Sprite } from './Sprite'
import './styles.css'

type ViewName = 'find' | 'scout'

export default function App() {
  const [view, setView] = useState<ViewName>('find')
  const [scoutRequest, setScoutRequest] = useState<ScoutRequestState | undefined>(undefined)
  const engine = useEngineInfo()
  const running = useStore(searchStore, (state) => state.state === 'running')

  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (searchStore.state.state === 'running') event.preventDefault()
    }
    window.addEventListener('beforeunload', warn)
    return () => window.removeEventListener('beforeunload', warn)
  }, [])

  useEffect(() => {
    document.title = view === 'scout' ? 'Scout a seed — Seed Seeker' : 'Seed Seeker — find your run'
  }, [view])

  const goScout = (seed: string) => {
    setScoutRequest({ seed, nonce: Date.now() })
    setView('scout')
  }

  return (
    <div className="d2-root">
      <header className="d2-topbar">
        <div className="d2-topbar-inner">
          <button type="button" className="d2-wordmark" onClick={() => setView('find')}>
            <span className="d2-wordmark-icon"><Sprite index={wildcardSprites.wand} size={20} /></span>
            <span className="d2-wordmark-text">
              Seed<em>Seeker</em>
              <span className="d2-wordmark-sub">expedition planner</span>
            </span>
          </button>
          <nav className="d2-nav" aria-label="Primary">
            <button type="button" className={view === 'find' ? 'is-active' : undefined} onClick={() => setView('find')}>
              Find
              {running && <span className="d2-nav-dot" title="Search running" />}
            </button>
            <button type="button" className={view === 'scout' ? 'is-active' : undefined} onClick={() => setView('scout')}>
              Scout
            </button>
          </nav>
        </div>
      </header>

      <main className="d2-main">
        <div hidden={view !== 'find'}><FindView onScout={goScout} /></div>
        <div hidden={view !== 'scout'}><ScoutView request={scoutRequest} /></div>
      </main>

      <footer className="d2-footer">
        <p>
          Unofficial companion for <strong>Shattered Pixel Dungeon v{engine?.shpdVersion ?? '…'}</strong> — a fan-made
          tool, not endorsed by the game's developers.
        </p>
        <p>
          GPL-3.0-or-later · <a href="/licenses/COPYING.txt">License</a> ·{' '}
          <a href="/third_party/shattered-pixel-dungeon/ATTRIBUTION.md">Game asset attribution</a>
        </p>
      </footer>
    </div>
  )
}
