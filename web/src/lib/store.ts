import { Store } from '@tanstack/store'
import { defaultQueryState, fromQueryJson, toQueryJson } from './query'
import type { QueryState } from './wasm/types'

const QUERY_KEY = 'seedseeker.query.v1'
const PRESETS_KEY = 'seedseeker.presets.v1'

function hydrateQuery(): QueryState {
  if (typeof localStorage === 'undefined') return defaultQueryState()
  try {
    const saved = localStorage.getItem(QUERY_KEY)
    return saved ? fromQueryJson(saved) : defaultQueryState()
  } catch {
    return defaultQueryState()
  }
}

export const queryStore = new Store<QueryState>(hydrateQuery())
if (typeof localStorage !== 'undefined') {
  queryStore.subscribe(() => localStorage.setItem(QUERY_KEY, toQueryJson(queryStore.state)))
}

export interface Preset { name: string; query: QueryState }
export const builtInPresets: Preset[] = [
  {
    name: '+3 Wand of Fireblast',
    query: { ...defaultQueryState(), requirements: [{ kind: 'wand', item: 'wand_fireblast', tier: { mode: 'any', value: 3 }, upgrade: { mode: 'at_least', value: 3 }, uncursed: false }] },
  },
  {
    name: 'Early +2 armor',
    query: { ...defaultQueryState(), requirements: [{ kind: 'armor', tier: { mode: 'any', value: 3 }, upgrade: { mode: 'at_least', value: 2 }, uncursed: false, maxDepth: 6 }] },
  },
]

export function loadPresets(): Preset[] {
  try {
    const value = localStorage.getItem(PRESETS_KEY)
    if (!value) return []
    const raw = JSON.parse(value) as { name: string; query: unknown }[]
    return raw.map((preset) => ({ name: preset.name, query: fromQueryJson(JSON.stringify(preset.query)) }))
  } catch {
    return []
  }
}

export function savePresets(presets: Preset[]): void {
  localStorage.setItem(PRESETS_KEY, JSON.stringify(presets.map((preset) => ({ name: preset.name, query: JSON.parse(toQueryJson(preset.query)) }))))
}
