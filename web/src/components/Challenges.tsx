import { challenges as challengeOptions } from '../lib/catalog'
import type { ChallengeName } from '../lib/wasm/types'

export function Challenges({ selected, onChange }: { selected: ChallengeName[]; onChange: (next: ChallengeName[]) => void }) {
  const toggle = (value: ChallengeName) => onChange(selected.includes(value) ? selected.filter((item) => item !== value) : [...selected, value])
  return (
    <details className="challenge-picker">
      <summary>Challenges {selected.length > 0 && <span className="count-badge">{selected.length}</span>}</summary>
      <div className="checkbox-list">
        {challengeOptions.map((challenge) => (
          <label className="check-row" key={challenge.value}>
            <input type="checkbox" checked={selected.includes(challenge.value)} onChange={() => toggle(challenge.value)} />
            <span>{challenge.label}</span>
          </label>
        ))}
      </div>
    </details>
  )
}
