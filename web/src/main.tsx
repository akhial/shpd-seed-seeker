import { StrictMode, Suspense, lazy } from 'react'
import { createRoot } from 'react-dom/client'

const designs = {
  one: lazy(() => import('./designs/one/App')),
  two: lazy(() => import('./designs/two/App')),
} as const

type DesignName = keyof typeof designs

function activeDesign(): DesignName {
  const fromUrl = new URLSearchParams(location.search).get('design')
  if (fromUrl === 'one' || fromUrl === 'two') {
    localStorage.setItem('seedseeker.design', fromUrl)
    return fromUrl
  }
  const saved = localStorage.getItem('seedseeker.design')
  return saved === 'two' ? 'two' : 'one'
}

function switchDesign(name: DesignName): void {
  localStorage.setItem('seedseeker.design', name)
  const url = new URL(location.href)
  url.searchParams.set('design', name)
  // Full reload so only the active design's stylesheets are ever loaded.
  location.href = url.toString()
}

function DesignSwitcher({ current }: { current: DesignName }) {
  return (
    <div
      style={{
        position: 'fixed', bottom: 12, right: 12, zIndex: 9999, display: 'flex', gap: 2,
        background: 'rgba(20, 24, 28, 0.92)', border: '1px solid rgba(255,255,255,0.18)',
        borderRadius: 999, padding: 3, font: '12px system-ui, sans-serif',
        boxShadow: '0 2px 12px rgba(0,0,0,0.4)',
      }}
    >
      {(Object.keys(designs) as DesignName[]).map((name) => (
        <button
          key={name}
          onClick={() => name !== current && switchDesign(name)}
          style={{
            border: 'none', borderRadius: 999, padding: '4px 12px', cursor: 'pointer',
            background: name === current ? '#3d8f6f' : 'transparent',
            color: name === current ? '#fff' : 'rgba(255,255,255,0.65)',
            font: 'inherit',
          }}
        >
          Design {name === 'one' ? '1' : '2'}
        </button>
      ))}
    </div>
  )
}

const design = activeDesign()
const App = designs[design]

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Suspense fallback={null}>
      <App />
    </Suspense>
    <DesignSwitcher current={design} />
  </StrictMode>,
)
