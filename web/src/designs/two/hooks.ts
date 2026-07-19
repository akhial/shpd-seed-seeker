import { useEffect, useState } from 'react'
import { getEngineInfo } from '../../lib/wasm'
import type { EngineInfo } from '../../lib/wasm/types'

let cachedEngineInfo: EngineInfo | undefined

export function useEngineInfo(): EngineInfo | undefined {
  const [info, setInfo] = useState(cachedEngineInfo)
  useEffect(() => {
    if (cachedEngineInfo) return
    let alive = true
    void getEngineInfo().then((value) => {
      cachedEngineInfo = value
      if (alive) setInfo(value)
    }).catch(() => undefined)
    return () => { alive = false }
  }, [])
  return info
}

export function useDebounced<T>(value: T, delay = 250): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delay)
    return () => window.clearTimeout(timer)
  }, [value, delay])
  return debounced
}
