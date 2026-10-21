# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

fastload is a small Windows desktop file downloader: paste a link, pick a target, download with a progress bar. HTTP downloads use `ureq`, torrents (magnet links, `.torrent` URLs/files) use `librqbit`, the GUI is `wxdragon` (wxWidgets bindings). UI strings are German.

## Commands

- Build: `cargo build`
- Run: `cargo run`
- Lint: `cargo clippy`

There are no tests yet. The build prints a German MSVC linker message ("Bibliothek ... und Objekt ... werden erstellt") as a warning; it is harmless.

## Architecture

- `src/main.rs` — GUI and HTTP download. `build_ui()` returns `(Frame, Ui)`; `Ui` holds only the widgets the download thread touches (text field, button, gauge, status). The click handler dispatches on `torrent::is_torrent()` to either a save-as dialog + HTTP download or a folder dialog + torrent download.
- `src/torrent.rs` — torrent download. `download()` creates its own tokio runtime and blocks on it, so it can be called from a plain `std::thread`. It starts a librqbit `Session` in the chosen folder, polls `stats()` every 500 ms and stops the session when finished (no seeding).
- `build.rs` — embeds the Windows manifest (common controls v6, UTF-8 code page, per-monitor DPI, long paths) via `embed-manifest`.

### Threading rules

- Downloads run on a `std::thread`; UI updates from there must go through `wxdragon::call_after`.
- wxdragon widgets like `Button`, `Gauge`, `TextCtrl`, `StaticText` are `Copy + Send` (they wrap a `WindowHandle`), but `Frame` is **not** `Send` — never move it into a thread. That's why the frame is kept out of `Ui`.
- Both download backends share the signature `fn(input, target, on_progress: impl Fn(u64, Option<u64>)) -> Result<u64, String>`, so `report_progress` / `report_finished` in `main.rs` serve both. Keep that shape when adding a backend.

### API notes (versions in Cargo.toml)

- librqbit 9: `Session::add_torrent` needs `&Arc<Session>`; the handle type is `Arc<ManagedTorrent>` (`ManagedTorrentHandle` is not exported). `AddTorrent::from_cli_argument` accepts magnet links, URLs and local paths.
- wxdragon `Gauge` has no `pulse()`.

## Style

Keep code simple and functional, split into many small free functions rather than structs/traits.
