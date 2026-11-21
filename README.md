# Flux

[![CI](https://github.com/xZeroW/tui-download-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/xZeroW/tui-download-manager/actions/workflows/ci.yml)

A fast, keyboard-driven terminal download manager written in Rust, built with [ratatui](https://ratatui.rs).

<p align="center">
  <img src="assets/demo.gif" alt="Flux demo">
</p>

## Features

- **Direct downloads** from any HTTP/HTTPS URL
- **Torrents** via magnet links or `.torrent` URLs (powered by [librqbit](https://github.com/ikatson/rqbit))
- **Google Drive** — sign in with OAuth, browse your files and download a selection
- **MediaFire** — public file and folder links
- Live progress for every download, shown as a bar or as text
- Command palette, file picker and vim-style navigation
- Credentials stored in your OS keyring, never in plain text

## Installation

Requires a recent stable Rust toolchain (edition 2024).

```bash
git clone https://github.com/xZeroW/tui-download-manager.git
cd tui-download-manager
cargo run --release -p dm-core
```

Or install the binary:

```bash
cargo install --path dm-core
```

## Usage

Press `:` to open the command palette and pick a provider:

| Command           | Description                                         |
| ----------------- | --------------------------------------------------- |
| `direct-download` | Download a file from an HTTP/HTTPS URL              |
| `torrent`         | Download from a magnet link or `.torrent` URL       |
| `mediafire`       | Download a file or folder from a public MediaFire link |

Cloud providers such as Google Drive appear in the sidebar. Select one, enter your credentials (for Google Drive: a client ID and secret), log in, then browse and mark the files you want.

### Keybindings

| Key                | Action                                   |
| ------------------ | ---------------------------------------- |
| `:`                | Open command palette                     |
| `Tab` / `h` / `l`  | Switch focus between sidebar and content |
| `j` / `k` / arrows | Move selection                           |
| `Enter`            | Activate / confirm                       |
| `Space`            | Toggle a setting or select a file        |
| `a`                | Select all (in the file picker)          |
| `g`                | Download selected cloud files            |
| `p`                | Pause / resume a download                |
| `d`                | Remove a download from the list          |
| `D`                | Delete a download (asks to confirm)      |
| `Esc`              | Close popup / go back                    |
| `q`                | Quit                                     |

### Settings

Settings live in the app's Settings screen and are saved to `config.toml` in your OS config directory (e.g. `~/.config/flux/config.toml` on Linux):

```toml
skip_splash = false
progress_display = "bar" # or "text"
```

## Project layout

A Cargo workspace with three crates:

- `dm-core` — application state, input handling, settings and the download store (the binary)
- `dm-providers` — download providers (direct, torrent, Google Drive, MediaFire)
- `dm-ui` — screens and widgets

Adding a provider means implementing the `DownloadProvider` trait in `dm-providers` and registering it in `providers()`.

## Development

```bash
cargo test --workspace
```

## License

Released under the [MIT License](LICENSE).
