import { ringIconCss, spriteBoxCss } from '../../lib/sprites'

export function Sprite({ index, size = 24 }: { index: number; size?: number }) {
  const box = spriteBoxCss(index, size)
  const ringIcon = ringIconCss(index, size)
  return (
    <span aria-hidden className="d2-sprite" style={box.outer}>
      <span style={box.inner} />
      {ringIcon && <span style={ringIcon} />}
    </span>
  )
}
