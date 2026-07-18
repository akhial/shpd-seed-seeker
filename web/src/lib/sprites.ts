import type { CSSProperties } from 'react'

export function spriteCss(index: number, size: number): CSSProperties {
  const scale = size / 16
  const col = index % 16
  const row = Math.floor(index / 16)
  return {
    width: `${size}px`,
    height: `${size}px`,
    backgroundImage: 'url(/third_party/shattered-pixel-dungeon/items.png)',
    backgroundPosition: `${-col * 16 * scale}px ${-row * 16 * scale}px`,
    backgroundSize: `${256 * scale}px auto`,
    imageRendering: 'pixelated',
    flex: '0 0 auto',
  }
}
