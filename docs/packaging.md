# Portable binaries (`cargo xtask package`)

One file per OS that plays the TUI with no game install and no data files: the game text and
art from `cargo xtask art-import` are built in (`sts2-tui`'s `embedded` feature,
`crates/sts2-tui/build.rs`). **Personal use only**: the binaries hold the game's assets, so
never commit, upload or publish them (`target/` is ignored).

```
cargo xtask art-import          # once per game version: art and text into ~/.cache/spirezero-data
cargo xtask package             # all four; or name some: linux windows macos-arm macos-x86
```

Output in `target/package/`: `sts2-tui-linux-x86_64` (static musl), `sts2-tui-windows-x86_64.exe`,
`sts2-tui-macos-arm64`, `sts2-tui-macos-x86_64`, about 22-28 MB each (17 MB of it art).

The Linux binary is profile-guided (PGO, PF15): `package` first builds `sts2-bench` instrumented for
the same target and profile, plays the bench's training workload on it (2.5 minutes), and builds the
TUI with that profile, which made the engine 18% faster in that build. A profile applies only to
builds of the target it was trained on, and a target is trained by running its own binary, so the
Windows and macOS binaries built here are plain fat-LTO builds (`--no-pgo` makes the Linux one plain
too). Built on a Mac, `cargo xtask package macos-arm` would train and use a profile for it the same
way (untested).

The TUI keeps no saves or settings, so it writes nothing and the binary can live anywhere.
`STS2_LOCALIZATION` and `STS2_ART` still override the built-in text and art.

## Tools (on the Linux/WSL build machine)
```
rustup target add x86_64-unknown-linux-musl x86_64-pc-windows-gnu aarch64-apple-darwin x86_64-apple-darwin
sudo apt-get install mingw-w64 zig      # zig 0.14 from Ubuntu's archive
cargo install --locked cargo-zigbuild
```
The macOS binaries are linked by zig with no macOS SDK (they need only `libSystem`, `libiconv`
and `libcharset`), and zig signs them ad hoc, which Apple silicon requires.

## On the Mac
Copy the file and run it:
```
chmod +x sts2-tui-macos-arm64
xattr -d com.apple.quarantine sts2-tui-macos-arm64   # only if it came via a browser or AirDrop
./sts2-tui-macos-arm64
```
If macOS still refuses it ("cannot be verified"), re-sign it ad hoc there: `codesign --force -s - sts2-tui-macos-arm64`.

To build on the Mac instead (no CI: the assets never leave your machines): install Rust, copy
`~/.cache/spirezero-data/art/art.bundle` and `~/.cache/spirezero-data/text/` to the same paths on
the Mac (or anywhere, and set `SPIREZERO_DATA` to it), then
`cargo build --profile portable -p sts2-tui --features embedded` → `target/portable/sts2-tui`.
