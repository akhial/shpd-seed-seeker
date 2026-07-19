import type { ReactNode } from 'react'

export interface SegmentOption<T> { value: T; label: string }

export function Segmented<T extends string | number>({ label, options, value, onChange, compact = false }: {
  label: string
  options: SegmentOption<T>[]
  value: T
  onChange: (value: T) => void
  compact?: boolean
}) {
  return (
    <div className={`d2-seg${compact ? ' d2-seg-compact' : ''}`} role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          className={option.value === value ? 'is-on' : undefined}
          aria-pressed={option.value === value}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}

export function Field({ label, children, wide = false }: { label: string; children: ReactNode; wide?: boolean }) {
  return (
    <div className={`d2-field${wide ? ' d2-field-wide' : ''}`}>
      <span className="d2-field-label">{label}</span>
      {children}
    </div>
  )
}

export function SelectShell({ children }: { children: ReactNode }) {
  return <span className="d2-select">{children}</span>
}

export function Switch({ label, caption, checked, disabled = false, onChange }: {
  label: string
  caption?: string
  checked: boolean
  disabled?: boolean
  onChange: (checked: boolean) => void
}) {
  return (
    <label className={`d2-switch-row${disabled ? ' is-disabled' : ''}`}>
      <input
        type="checkbox"
        className="d2-switch"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span className="d2-switch-text">
        <span className="d2-switch-label">{label}</span>
        {caption && <span className="d2-switch-caption">{caption}</span>}
      </span>
    </label>
  )
}
