# Seed Seeker web app

Run the web toolchain from this directory with `vp`. On a fresh checkout, build
the browser engine and generated assets first:

```sh
../scripts/build-web-wasm.sh
vp install
vp dev
```

Use `vp check`, `vp test`, and `vp build` to run the same web checks as CI.

## Source layout

```text
src/
  main.tsx             Browser entry point
  app/                 App shell, global styles, and persisted application state
  engine/              WASM bindings, engine types, and contract tests
    pkg/               Generated wasm-pack output (gitignored)
  features/
    query/             Query form, serialization, validation, and share links
      requirements/    Requirement board, editor, relationships, and summaries
    results/           Result list, import/export, and result navigation
    search/            Search coordination, workers, progress, and status UI
    scout/             Seed and daily-run scouting, floor summaries
      seed-info/       Seed identity mappings and their dialog
      trinkets/        Trinket artwork, shortcuts, and dock behavior
    level-map/         Interactive maps, gestures, and map requests
      rendering/       Canvas rendering, particles, caching, and render worker
  shared/
    game/              Catalog, floor requirements, quests, and regions
    sprites/           Sprite geometry and item glow data
    ui/                Reusable UI primitives, icons, and feeling artwork
    format.ts          Shared number formatting
  assets/              Checked-in artwork and attribution
  generated/           Generated catalog and sprite metadata (gitignored)
```

Keep feature components, helpers, styles, and tests together. Shared code belongs
in the named `shared/` area that describes its purpose. `app/store.ts` owns the
persisted query, presets, and worker settings used across features. Tests sit
beside the code they exercise; tests spanning query editing and engine behavior
live with the query feature.

Use direct module imports. Keep worker entry points and their `new URL(...,
import.meta.url)` references together when moving code so Vite can bundle them.
The WASM build script owns `engine/pkg/`, `generated/`, and the generated runtime
assets under `public/`; rerun it after Rust engine changes.

Global styles remain in `app/styles.css`, while map and seed-info styles live
with their features. The existing CSS selectors and import order are preserved.
