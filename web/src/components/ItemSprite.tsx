import { spriteCss } from '../lib/sprites'

export function ItemSprite({ spriteIndex, size = 32, name, muted = false }: { spriteIndex: number; size?: number; name: string; muted?: boolean }) {
  return <span className="item-sprite" role="img" aria-label={name} style={{ ...spriteCss(spriteIndex, size), opacity: muted ? 0.6 : 1 }} />
}
