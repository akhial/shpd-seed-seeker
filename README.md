<p align="center">
  <img alt="Seed Seeker app icon" src="assets/icon/seed-seeker-square.svg" width="128" height="128">
</p>

<h1 align="center">Seed Seeker</h1>

<p align="center">
  <a href="https://github.com/akhial/shpd-seed-seeker/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/akhial/shpd-seed-seeker/actions/workflows/ci.yml/badge.svg"></a>
  <a href="COPYING"><img alt="License: GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg"></a>
</p>

An extremely fast seed finder for [Shattered Pixel Dungeon](https://shatteredpixel.com/),
written in Rust — with native apps for Android, Linux, macOS, and Windows.

**[Try it in your browser →](https://shpd-seed-seeker.web.app/)**

<p align="center">
  <img alt="Matching seeds per minute on an Apple M4 Pro, 12 workers. +2 Runic Blade (Grim, Corrupting, Vampiric or Crystal) and +2 Ring of Might: Seed Seeker auto 79.9, Java 10.6. +5 Crossbow: Seed Seeker auto 8,429.7, Java 1,772.0. Both queries through floor 19; separate bar scales." src="assets/benchmark.svg">
</p>

<p align="center">
  <i>Matches/minute on an Apple M4 Pro (12 cores), 10 minutes per mode. Seed Seeker uses auto-apply; each query has its own bar scale.</i>
</p>

- ⚡️ **4.8–7.5× Java’s match throughput** in the [native benchmark](#benchmarks)
- 🔍 **Rich queries**: multiple requirements across melee and thrown weapons, armor, wands, and rings
- 🔗 **Share links**: any search fits in a short link that fills in the query on every platform
- 🔮 **Seed scouting**: paste a seed, get every item with floor, upgrade, enchantment, cursed state and source
- 📱 **Native apps** Material 3, GTK 4 and libadwaita, SwiftUI, WinUI 3

## Table of contents

1. [Getting started](#getting-started)
1. [Search queries](#search-queries)
1. [Benchmarks](#benchmarks)
1. [Development](#development)
1. [Acknowledgements](#acknowledgements)
1. [License and identity](#license-and-identity)

## Getting started<a id="getting-started"></a>

### Web app

Run searches directly at
[shpd-seed-seeker.web.app](https://shpd-seed-seeker.web.app/).

### Download a release

Binaries are published on the [GitHub Releases page](https://github.com/akhial/shpd-seed-seeker/releases).

| Asset | Platforms |
| --- | --- |
| `seed-seeker-cli-<tag>-<target>.tar.gz` / `.zip` | CLI for Linux (x86_64, arm64), macOS (Apple Silicon, Intel), and Windows (x86_64, arm64) |
| `seed-seeker-cli-<tag>-<target>-avx2.tar.gz` / `.zip` | CLI built for AVX2 x86-64 machines |
| `seed-seeker-<tag>-<arch>.AppImage` | Native Linux app (x86_64, arm64) |
| `seed-seeker-<tag>-macos-arm64.dmg` | Native macOS app (Apple Silicon, macOS 14+) |
| `seed-seeker-<tag>-windows-<arch>.zip` | Native Windows app (x64, ARM64) |
| `seed-seeker-<tag>-windows-x64-avx2.zip` | Windows app built for AVX2 x86-64 machines |
| `seed-seeker-<tag>-android.apk` | Android app (arm64-v8a and x86_64) |

- The Windows app requires the [Windows App SDK 1.8 runtime](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/downloads).

### CLI

Build and run a quick CLI benchmark:

```sh
cargo run --release -p shpd-seedfinder-cli -- --benchmark
cargo run --release -p shpd-seedfinder-cli -- -b 1000 --workers 4
```

To search, put the requirements in a JSON file and pass it with `--items` (or `-i`):

```sh
cargo run --release -p shpd-seedfinder-cli -- --items requirements.json
cargo run --release -p shpd-seedfinder-cli -- -i requirements.json -b 1000 --workers 4
```

## Search queries<a id="search-queries"></a>

```jsonc
// ? optional · | alternatives · .. inclusive range · = default · ... repeat
{
  "max_depth"?: 1..24 = 24,
  "require_blacksmith"?: true | false = false,
  "exclude_blacksmith_rewards"?: true | false = false,
  // The run's Wandmaker (floors 7-9) must ask for this quest item.
  "wandmaker_quest"?: "corpse_dust" | "elemental_embers" | "rotberry",

  "challenges"?: [
    (
      "on_diet" | "faith_is_my_armor" | "pharmacophobia" | "barren_land" |
      "swarm_intelligence" | "into_darkness" | "forbidden_runes" |
      "hostile_champions" | "badder_bosses"
    ),
    ...
  ] = [],

  "requirements": [
    // Each entry is a requirement, or an "any_of" group that is satisfied
    // when any single member matches:
    //   { "any_of": [ { "item": "spear", "upgrade": 3 },
    //                 { "item": "shuriken", "upgrade": 2 },
    //                 { "item": "sword", "upgrade": 1 } ] }
    // Members use the requirement schema below, except "level_sum".
    {
      // Supply "item", "kind", or both; when both are present they must agree.
      // "weapon" matches melee and thrown weapons alike; "melee_weapon" and
      // "thrown_weapon" narrow it to one class.
      "kind"?: "weapon" | "melee_weapon" | "thrown_weapon" | "armor" | "wand" | "ring",
      "item"?:
        // Weapons
        "worn_shortsword" | "cudgel" | "gloves" | "rapier" | "dagger" |
        "shortsword" | "hand_axe" | "spear" | "quarterstaff" | "dirk" | "sickle" |
        "sword" | "mace" | "scimitar" | "round_shield" | "sai" | "whip" |
        "longsword" | "battle_axe" | "flail" | "runic_blade" | "assassins_blade" |
        "crossbow" | "katana" | "greatsword" | "war_hammer" | "glaive" | "greataxe" |
        "greatshield" | "gauntlet" | "war_scythe" | "throwing_stone" |
        "throwing_knife" | "throwing_spike" | "fishing_spear" | "throwing_club" |
        "shuriken" | "throwing_spear" | "kunai" | "bolas" | "javelin" | "tomahawk" |
        "heavy_boomerang" | "trident" | "throwing_hammer" | "force_cube" | "rot_dart" |
        "incendiary_dart" | "adrenaline_dart" | "healing_dart" | "chilling_dart" |
        "shocking_dart" | "poison_dart" | "cleansing_dart" | "paralytic_dart" |
        "holy_dart" | "displacing_dart" | "blinding_dart" |
        // Armor
        "cloth_armor" | "leather_armor" | "mail_armor" | "scale_armor" | "plate_armor" |
        // Wands
        "wand_magic_missile" | "wand_fireblast" | "wand_frost" | "wand_lightning" |
        "wand_disintegration" | "wand_prismatic_light" | "wand_corrosion" |
        "wand_living_earth" | "wand_blast_wave" | "wand_corruption" | "wand_warding" |
        "wand_regrowth" | "wand_transfusion" |
        // Rings
        "ring_accuracy" | "ring_arcana" | "ring_elements" | "ring_energy" |
        "ring_evasion" | "ring_force" | "ring_furor" | "ring_haste" | "ring_might" |
        "ring_sharpshooting" | "ring_tenacity" | "ring_wealth",

      // Tier filters apply only to wildcard weapon/armor requirements.
      "tier"?:
        "any" |
        { "exact": 2..5 } |
        { "at_least": 3..4 } |
        { "at_most": 3..4 }
        = "any",

      // Everything reaches +4; a tier-4 weapon, melee or thrown, reaches +5.
      // "any" and effect names are case-insensitive.
      "upgrade"?:
        "any" | 1..5 |
        { "exact": 1..5 } |
        { "at_least": 0..5 }
        = "any",

      // The effect must belong to the selected weapon or armor kind. A list
      // matches any one of its entries, and "any_enchantment" is shorthand
      // for every non-curse enchantment or glyph.
      "effect"?: <name> | [<name>, ...] | "any_enchantment", where <name> is
        // Weapon enchantments
        "Blazing" | "Chilling" | "Kinetic" | "Shocking" | "Blocking" | "Blooming" |
        "Elastic" | "Lucky" | "Projecting" | "Unstable" | "Corrupting" | "Grim" |
        "Vampiric" | "Venomous" | "Eldritch" | "Vorpal" | "Crystal" |
        // Weapon curses
        "Annoying" | "Displacing" | "Dazzling" | "Explosive" | "Sacrificial" |
        "Wayward" | "Polarized" | "Friendly" | "Pressurized" | "Wondrous" |
        // Armor glyphs
        "Obfuscation" | "Swiftness" | "Viscosity" | "Potential" | "Brimstone" | "Stone" |
        "Entanglement" | "Repulsion" | "Camouflage" | "Flow" | "Affection" |
        "Anti-Magic" | "Thorns" |
        // Armor curses
        "Anti-Entropy" | "Corrosion" | "Displacement" | "Metabolism" | "Multiplicity" |
        "Stench" | "Overgrowth" | "Bulk",

      // true cannot be combined with a curses-only effect list.
      "uncursed"?: true | false = false,
      "source"?:
        "heap" | "chest" | "locked_chest" | "crystal_chest" | "tomb" | "skeleton" |
        "sacrificial_fire" | "mimic" | "golden_mimic" | "crystal_mimic" | "statue" |
        "armored_statue" | "shop" | "ghost_reward" | "wandmaker_reward" |
        "blacksmith_reward" |
        // The Imp's six vault prizes, and the equipment in the vault's
        // treasure rooms; the player carries exactly one item out of either.
        "imp_reward" | "vault_treasure",
      // Equal groups must resolve to the same kind and item ID.
      "identity_group"?: 1..255,
      "max_depth"?: 1..24 = query.max_depth,
      // Requirements sharing a group are matched by distinct items whose
      // *levels* — each item's upgrade plus one — add up to at least
      // "at_least", on top of each member's own upgrade filter. Members are
      // optional: any subset that reaches the total satisfies the group, so
      // "up to two Rings of Might reaching 5 levels" (a +1 and a +2, or a
      // single +4) is:
      //   { "item": "ring_might", "level_sum": { "group": 1, "at_least": 5 } },
      //   { "item": "ring_might", "level_sum": { "group": 1, "at_least": 5 } }
      // All members of one group must agree on "at_least". A same-item group
      // ("identity_group") is a stack: one member — or the members of one
      // "any_of" group — may name the item and its qualities; every other
      // member must be a plain entry of the same kind.
      "level_sum"?: { "group": 1..255, "at_least": 1..255 }
    },
    ...
  ]
}
```

## Benchmarks<a id="benchmarks"></a>

**Matching seeds per minute**, through floor 19. The compound query requires an
exact +2 Runic Blade with **Grim, Corrupting, Vampiric or Crystal**, and an exact
+2 Ring of Might. The control requires an exact +5 Crossbow. No challenges.

| Query | Java baseline | Seed Seeker, auto off | Seed Seeker, auto on | Auto / Java |
| --- | ---: | ---: | ---: | ---: |
| [+2 Runic Blade + +2 Ring of Might](https://shpd-seed-seeker.web.app/#q=QyAhKCsAAeAAAuoKAA) | 10.6 | 54.9 | **79.9** | 7.5× |
| +5 Crossbow (control) | 1,772.0 | 8,398.0 | **8,429.7** | 4.8× |

Auto-apply improved compound-query match throughput by **45.4%**
(95% paired block bootstrap interval: 37.7–53.7%). On identical seeds it
found 313 additional matches and lost 65. The Crossbow control selected no trinket
and returned identical match sets: auto/off throughput was 1.004×
(95% interval 0.999–1.008×).

- **Machine:** Apple M4 Pro (8 performance + 4 efficiency cores), 48 GB, macOS 26.6.2; 12 native workers versus 12 persistent JVMs.
- **Builds:** Rust 1.98.0, `aarch64-apple-darwin`, release/fat LTO, mimalloc, fresh PGO trained on disjoint seeds; OpenJDK 26.0.2.1 and the unmodified official Shattered Pixel Dungeon **4.0.0 JAR (build 912)**, pinned in [the Java adapter](tooling/java-finder).
- **Method:** at least 600 timed seconds per query/mode, finishing the last block; 1,024 warm-up seeds per worker plus a regression seed. Alternating mode order, with only one mode running at a time; native off/auto search identical dispersed seeds. Java searches a shorter prefix of the same sequence. Times include batch dispatch and result collection, excluding builds, startup, warm-up and verification.

| Query | Seeds: Java / each native mode | Matches: Java / off / auto | Seconds: Java / off / auto |
| --- | ---: | ---: | ---: |
| Blade + ring | 595,968 / 2,899,968 | 106 / 552 / 800 | 600.5 / 603.0 / 601.1 |
| Crossbow | 737,280 / 3,526,656 | 17,759 / 84,532 / 84,532 | 601.3 / 603.9 / 601.7 |

Every native result passed a full floor-24 query replay; every reported result's
supporting items were independently replayed in the JAR, checking floor, source,
item, upgrade, cursed state and effect. Java and native off agreed on the entire
Java sample, including negative seeds. Automatic recipes also passed the JAR's
initial-offer check. Trinkets use the engine's **+3 after the first catalyst/alchemy
opportunity** profile; these are warmed native measurements, not browser latency.

Reproduce on macOS arm64 with Rust's `llvm-tools` component and a JDK on `PATH`
(about an hour of timing, plus training, warm-up and replay):

```sh
tooling/benchmarks/run.sh --minutes 10 --workers 12 --output /tmp/seed-seeker-benchmark
```

The script builds both adapters, runs both queries, validates matches and writes
raw blocks, counts, timings, source/binary/profile hashes and 5,000-resample
bootstrap intervals to a new output directory. No generated results are committed.

## Development<a id="development"></a>

### Web app

The web app is a [Vite+](https://viteplus.dev) project. Install the `vp` CLI once
(`curl -fsSL https://vite.plus | bash`), then build the browser engine before starting the dev
server:

```sh
./scripts/build-web-wasm.sh && cd web && vp install && vp dev
```

`vp check` (format, lint, type-check), `vp test`, and `vp build` cover the rest; they all need the
generated engine assets, so run `build-web-wasm.sh` first on a fresh clone.

### Android

#### Building

```sh
JAVA_HOME=/path/to/java-21 ./android/gradlew -p android :app:assembleRelease
```

#### Signing

```sh
"$ANDROID_HOME/build-tools/36.1.0/apksigner" sign \
  --ks "$HOME/.android/debug.keystore" \
  --ks-pass pass:android --key-pass pass:android \
  --out seed-seeker-release-debug-signed.apk \
  android/app/build/outputs/apk/release/app-release-unsigned.apk
```

#### Installing

```sh
"$ANDROID_HOME/platform-tools/adb" devices -l
"$ANDROID_HOME/platform-tools/adb" install -r seed-seeker-release-debug-signed.apk
"$ANDROID_HOME/platform-tools/adb" shell monkey \
  -p dev.seedseeker.unofficial -c android.intent.category.LAUNCHER 1
```

### macOS

#### Building

```sh
bash scripts/build-macos-native.sh
bash scripts/build-macos-app.sh
```

#### PGO

```sh
rustup component add llvm-tools && bash scripts/record-pgo-profile.sh
```

### Linux

The Linux app requires GTK 4.22, libadwaita 1.9, and `glib-compile-resources`;
[`linux/README.md`](linux/README.md) lists the development packages.

```sh
cargo run -p shpd-seedfinder-gtk
```

To build the AppImage on Fedora 44, install the packages from [`linux/README.md`](linux/README.md), plus `curl` and `file`, then run:

```sh
APPIMAGE_VERSION=dev bash scripts/build-linux-appimage.sh
./dist/seed-seeker-dev-"$(uname -m)".AppImage
```

### Windows

The Windows app requires Visual Studio with the WinUI application development and .NET desktop development workloads, and the Rust MSVC target (`rustup target add aarch64-pc-windows-msvc`); [`windows/README.md`](windows/README.md) lists the development packages.

```powershell
.\scripts\build-windows-app.ps1
```

The script builds for the host architecture; pass `-Platform ARM64` or `-Platform x64` to cross-build, and `-Configuration Debug` for a debug build.

Pass `-EngineIsa avx2` to build the engine for x86-64-v3.

#### PGO

```sh
PGO_TARGET=x86_64-pc-windows-msvc bash scripts/record-pgo-profile.sh
```

### Testing

#### Rust

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The workspace includes the GTK app, so the commands above need its system libraries (GTK 4.22 and libadwaita 1.9). Add `--exclude shpd-seedfinder-gtk` on macOS and Windows to exclude the GTK app from the test run.

#### Android

```sh
JAVA_HOME=/path/to/java-21 ./android/gradlew -p android \
  :app:testDebugUnitTest \
  :app:lintDebug \
  :app:assembleRelease
```

#### macOS

For the macOS app, build the Rust static library before running the Swift tests:

```sh
bash scripts/build-macos-native.sh
cd macos/SeedSeeker
swift test
```

#### Java Oracle

```sh
javac -d /tmp tooling/parity/RngOracle.java
java -cp /tmp RngOracle
```

`EquipmentOracle.java` is compiled against the isolated v3.3.8 JAR.
`tooling/oracle-4.0` drives the v4.0.0 release JAR headlessly.

## Acknowledgements<a id="acknowledgements"></a>

Seed Seeker reimplements the generation of
[Shattered Pixel Dungeon](https://github.com/00-Evan/shattered-pixel-dungeon) by Evan Debenham,
itself based on [Pixel Dungeon](https://github.com/watabou/pixel-dungeon) by Oleg Dolya.

## License and identity<a id="license-and-identity"></a>

This project is GPL-3.0-or-later.

- Pixel Dungeon © 2012–2015 Oleg Dolya / Watabou
- Shattered Pixel Dungeon © 2014–2026 Evan Debenham
