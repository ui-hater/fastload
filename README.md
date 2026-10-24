# fastload

Ein kleiner Datei-Downloader für Windows: Link einfügen, Ziel wählen, herunterladen – mit Fortschrittsbalken.

## Funktionen

- **HTTP/HTTPS-Downloads** über [`ureq`](https://crates.io/crates/ureq) – das Ziel wird per „Speichern unter“-Dialog gewählt.
- **Torrents** über [`librqbit`](https://crates.io/crates/librqbit) – Magnet-Links, `.torrent`-URLs und lokale `.torrent`-Dateien. Das Ziel ist ein Ordner; nach Abschluss wird die Sitzung beendet (kein Seeding).
- **Native Oberfläche** mit [`wxdragon`](https://crates.io/crates/wxdragon) (wxWidgets).
- **Übersetzungen** per gettext (`po/`, `locale/`); die Sprache richtet sich nach der Systemsprache.

## Voraussetzungen

- Windows
- Rust (Edition 2024) mit MSVC-Toolchain

## Bauen und Starten

```sh
cargo build --release
cargo run
```

Der Linker gibt beim Bauen eine Meldung „Bibliothek … und Objekt … werden erstellt“ aus – die ist harmlos.

Die Übersetzungen werden zur Laufzeit aus `.\locale` relativ zum Arbeitsverzeichnis geladen.

## Installer

Unter `wix/` liegt eine Vorlage für [`cargo wix`](https://github.com/volks73/cargo-wix):

```sh
cargo install cargo-wix
cargo wix
```

## Projektstruktur

| Pfad             | Inhalt                                                       |
| ---------------- | ------------------------------------------------------------ |
| `src/main.rs`    | Oberfläche und HTTP-Download                                 |
| `src/torrent.rs` | Torrent-Download mit librqbit                                |
| `build.rs`       | Bettet das Windows-Manifest ein (DPI, UTF-8, lange Pfade)    |
| `po/`, `locale/` | Übersetzungsvorlagen und kompilierte `.mo`-Kataloge          |
| `wix/`           | WiX-Installer                                                |

## Lizenz

MIT – siehe [LICENSE](LICENSE).
