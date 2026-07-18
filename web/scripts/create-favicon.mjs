import { spawnSync } from 'node:child_process'

const result = spawnSync('convert', [
  'public/third_party/shattered-pixel-dungeon/items.png',
  '-crop', '16x16+0+112', '+repage', '-filter', 'point', '-resize', '32x32',
  'public/favicon.png',
], { stdio: 'inherit' })
if (result.status !== 0) process.exit(result.status ?? 1)
