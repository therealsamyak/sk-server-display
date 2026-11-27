# SK Status Display

A fullscreen dashboard for the clock, system metrics, and service status.

[Screenshots](assets/screenshots/)

## Quick start

1. Download the build for your device from [GitHub Releases](https://github.com/therealsamyak/sk-server-display/releases) and open it. The dashboard starts with the clock and a local device card.
2. Copy [config.example.toml](config.example.toml) to the path below for your OS. Create the folder if needed, then edit the file.
3. Press **R** in the app to load your settings.

| OS | Configuration path |
| --- | --- |
| Windows | `%APPDATA%\server-status-display\config.toml` |
| macOS | `~/Library/Application Support/server-status-display/config.toml` |
| Linux | `~/.config/server-status-display/config.toml` |

See the [configuration reference](docs/configuration.md) for all settings and the [LAN example](config.lan.example.toml) for a LAN-only setup. Select **Wallpaper** in the app to change the background.

## Build from source

Run `cargo run --release --locked`. On Linux, install the system libraries from the [Zed build guide](https://zed.dev/docs/development/linux) first.

[License](LICENSE.md) · [Wallpaper credits](WALLPAPERS.md)
