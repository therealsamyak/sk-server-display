# `config.toml`

The app works without a config file. To customize it, copy the [example config](../config.example.toml) to the path in the [quick start](../README.md#quick-start). Press **R** in the app after saving your edits.

Use `--config <file>` to select a different file. On Linux, if you set `XDG_CONFIG_HOME`, the default path is `$XDG_CONFIG_HOME/server-status-display/config.toml`.

## General

Put top-level settings before the first table.

### `preset_wallpapers`

Folder of JPEG or PNG presets for the Wallpaper menu. Relative paths start at the config file's folder. Name each image after its [style ID](../WALLPAPERS.md), such as `yolanda-kokomi.png`.

```toml
preset_wallpapers = "wallpapers"
```

### `direct_metrics_http`

Connect directly to Glances, Uptime Kuma, and Scrutiny. Default `false`. When enabled, the app skips proxies and requires URLs that work without redirects.

```toml
direct_metrics_http = true
```

For a LAN-only display, start with the [LAN example](../config.lan.example.toml) and use addresses reachable from the display. The clock, local metrics, and local wallpapers work offline.

## Host cards

Add one `[[host]]` block per card. Cards appear in file order. Without any host blocks, the app shows one local card named **This machine**.

### `host.name` and `host.source`

Both keys are required. `name` sets the card label. Choose a `source` for the information you want to see.

- `local` shows system metrics and power status for the display computer.
- `glances` shows system metrics for a remote host.
- `tailscale` shows a device's online status, OS, and last-seen time when available.

```toml
[[host]]
name = "This machine"
source = "local"
```

### `host.url`

Glances v4 base URL. Required for `source = "glances"`. Use an HTTP or HTTPS URL without credentials, query parameters, or a fragment.

```toml
[[host]]
name = "Server"
source = "glances"
url = "http://glances.example.com:61208"
```

### `host.fallback_device` and `host.show_btop`

Optional Glances settings. `fallback_device` shows the named Tailscale device's presence when Glances is unavailable. `show_btop` adds a **btop** button to the expanded card. Its default is `false`.

```toml
[[host]]
name = "Server"
source = "glances"
url = "http://glances.example.com:61208"
fallback_device = "server"
show_btop = true
```

### `host.device`

Tailscale device name. Required for `source = "tailscale"`. It can differ from the card label.

```toml
[[host]]
name = "Travel laptop"
source = "tailscale"
device = "laptop"
```

## Uptime Kuma

Add `[uptime_kuma]` to show the **Services** panel.

### `uptime_kuma.url` and `uptime_kuma.status_page`

Both keys are required. `url` is the Kuma base URL. `status_page` is the last part of the status page address, such as `display` for `/status/display`. Use a page the display can open without signing in. Add the monitors you want to see to that page.

```toml
[uptime_kuma]
url = "https://status.example.com"
status_page = "display"
```

## Scrutiny

Add `[scrutiny]` to show the **Disks** panel.

### `scrutiny.url`

Required Scrutiny base URL.

```toml
[scrutiny]
url = "https://disks.example.com"
```

## Tailscale

The `[tailscale]` table is optional.

### `tailscale.show_devices`

Show the **Tailnet** panel. Default `false`. Devices that already have their own cards are omitted.

```toml
[tailscale]
show_devices = true
```

### `tailscale.command`

Tailscale executable name or path. Default `"tailscale"`. Applies to the Tailnet panel and Tailscale host cards.

```toml
[tailscale]
command = "tailscale"
```

## Launch options

Run the executable with `--help` to list its flags or `--version` to see the installed version.

- `--config <file>` selects an existing config file.
- `--display <selector>` accepts `auto` (default), `primary`, or a number starting at `1`. On Windows, `auto` prefers a physical display.
- `--windowed` starts the app in a window. The default is fullscreen.
- `--export-profile <directory>` copies the config, artwork, and slideshow settings to a portable directory.
- `--import-profile <directory>` restores a portable profile. Use `--config` to select a new path.
