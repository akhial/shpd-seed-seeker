# Seed Seeker for Linux

This directory contains the native Linux application. It uses GTK 4 through gtk-rs and
libadwaita-rs, follows the GNOME application-ID and resource conventions, and is part of the root
Cargo workspace. The binary is named `seed-seeker-gtk` because the CLI already claims
`seed-seeker` in the shared workspace target directory.

The app links the shared Rust engine in-process through `shpd-seedfinder-session`:

- **Search** takes the same JSON query format as the CLI (see the root README), runs a
  full-seed-space production session on all cores with a rotated start, and streams matching
  seed codes with live progress; activating a result row copies the seed code. Sessions are
  cancellable and cap accepted results at 1,024, like the other app frontends.
- **Scout** takes a seed code and lists every searchable item through depth 24 with floor,
  upgrade, enchantment or glyph, cursed state, source, transmuted Imp-ring identity, and
  choice constraints.

The shell also provides the application lifecycle, an adaptive libadwaita window, app actions,
an About dialog, embedded resources, a desktop entry, AppStream metadata, and full-color plus
symbolic icons.

## Requirements

- Rust 1.97 or newer
- GTK 4.22 or newer
- libadwaita 1.9 or newer
- `pkg-config` and `glib-compile-resources`

On Fedora 44:

```sh
sudo dnf install gcc gtk4-devel libadwaita-devel pkgconf-pkg-config
```

## Run

From the repository root:

```sh
cargo run -p shpd-seedfinder-gtk
```

GTK Inspector can be enabled while developing with `GTK_DEBUG=interactive`.

## Validate

```sh
cargo check -p shpd-seedfinder-gtk
cargo clippy -p shpd-seedfinder-gtk --all-targets -- -D warnings
desktop-file-validate linux/data/dev.seedseeker.SeedSeeker.desktop
appstreamcli validate --no-net linux/data/dev.seedseeker.SeedSeeker.metainfo.xml
```
