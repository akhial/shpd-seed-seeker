# Rust static-library prerequisite

SwiftPM links the production engine from
`../../target/aarch64-apple-darwin/release/libshpd_seedfinder_ffi.a`.
Build it before `swift build` or `swift test`:

```sh
cargo build --release --target aarch64-apple-darwin -p shpd-seedfinder-ffi
cd macos/SeedSeeker
swift test
```

`Package.swift` supplies the corresponding `-L` and
`-lshpd_seedfinder_ffi` linker flags. `CSeedFinder.h` includes the FFI crate's
handwritten header, keeping that header as the single source of truth.
