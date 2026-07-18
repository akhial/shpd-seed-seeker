import { describe, expect, it } from 'vitest'
import { spriteCss } from './sprites'

describe('spriteCss', () => {
  it.each([
    [0, '0px 0px'],
    [15, '-480px 0px'],
    [16, '0px -32px'],
    [96, '0px -192px'],
    [235, '-352px -448px'],
  ])('maps atlas index %i to its scaled position', (index, position) => {
    const style = spriteCss(index, 32)
    expect(style.backgroundPosition).toBe(position)
    expect(style.backgroundSize).toBe('512px auto')
  })
})
