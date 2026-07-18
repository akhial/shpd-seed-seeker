import { defineConfig } from 'vitest/config'

export default defineConfig({
  build: {
    lib: {
      entry: 'src/lib/wasm/index.ts',
      formats: ['es'],
      fileName: 'seed-seeker',
    },
  },
  test: {
    environment: 'node',
  },
})
