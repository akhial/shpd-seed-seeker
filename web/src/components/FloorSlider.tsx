import { floorLabel } from '../lib/region'

export function FloorSlider({ value, onChange, id, disabled = false }: { value: number; onChange: (value: number) => void; id: string; disabled?: boolean }) {
  return (
    <div className="floor-slider">
      <label htmlFor={id}>{floorLabel(value)}</label>
      <input id={id} type="range" min="1" max="24" value={value} disabled={disabled} onChange={(event) => onChange(event.currentTarget.valueAsNumber)} />
    </div>
  )
}
