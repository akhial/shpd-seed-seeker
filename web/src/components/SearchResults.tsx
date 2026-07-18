import { useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { CoordinatorState } from '../lib/search/coordinator-state'
import { compactNumber, formatDuration } from '../lib/format'

export function SearchResults({ search, onScout }: { search: CoordinatorState; onScout: (seed: string) => void }) {
  const parentRef = useRef<HTMLDivElement>(null)
  const [copied, setCopied] = useState<string>()
  const virtualizer = useVirtualizer({ count: search.matches.length, getScrollElement: () => parentRef.current, estimateSize: () => 44, overscan: 8 })
  const copy = async (code: string) => {
    await navigator.clipboard.writeText(code)
    setCopied(code)
    window.setTimeout(() => setCopied((current) => current === code ? undefined : current), 1_500)
  }
  const stateLine = search.state === 'idle'
    ? 'Add requirements, then start a search.'
    : `${search.state === 'running' ? 'Searching…' : search.state === 'completed' ? 'Search complete' : 'Search cancelled'} ${compactNumber(search.tested)} seeds · ${compactNumber(search.rate)}/s · ${formatDuration(search.elapsed)}`
  return (
    <main className="results-area">
      <section className="status-card" aria-live="polite">
        <div className="status-line">{search.state === 'running' && <span className="spinner" aria-hidden="true" />}<strong>{stateLine}</strong></div>
        <div className={`progress-track ${search.state === 'running' ? 'active' : ''}`}><span style={{ width: `${search.total ? Math.min(100, search.tested / search.total * 100) : 0}%` }} /></div>
      </section>
      {search.error && <div className="banner error">{search.error}</div>}
      {search.capped && <div className="banner warning">Result limit reached — showing the first 1,024 matches. Narrow the query to see others.</div>}
      <div className="results-heading"><div><p className="eyebrow">Discoveries</p><h1>{search.matches.length.toLocaleString()} seeds found</h1></div>{search.state === 'running' && <span className="dig-label">DIGGING</span>}</div>
      {search.state === 'running' && search.matches.length === 0 && <div className="empty-state"><span className="empty-pick">⌁</span><h2>No matches yet</h2><p>Keep digging. The dungeon is vast.</p></div>}
      {search.state === 'idle' && <div className="empty-state"><span className="empty-pick">⚒</span><h2>Ready to seek</h2><p>Shape a query on the left, then search every possible seed.</p></div>}
      {search.matches.length > 0 && <div ref={parentRef} className="virtual-list" aria-label="Matching seeds">
        <div style={{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }}>
          {virtualizer.getVirtualItems().map((row) => {
            const match = search.matches[row.index]
            return <div className="result-row" key={match.value} style={{ transform: `translateY(${row.start}px)` }}>
              <span className="seed-code">{match.code}</span>
              <div className="result-actions">
                <button type="button" className="icon-button" aria-label={`Copy ${match.code}`} onClick={() => void copy(match.code)} title={copied === match.code ? 'Copied' : 'Copy'}>{copied === match.code ? '✓' : '⧉'}</button>
                <button type="button" className="button ghost compact" onClick={() => onScout(match.code)}>Scout</button>
              </div>
            </div>
          })}
        </div>
      </div>}
      <span className="sr-only" aria-live="polite">{copied ? `${copied} copied` : ''}</span>
    </main>
  )
}
