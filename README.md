# Server status display

Server status display is a full-screen, continuously updating status board. It runs on the physical
monitor of a Windows laptop while Sunshine streams a game from a separate virtual display to
Moonlight. Windows is the priority platform. The same code builds for macOS and Linux.

The display reads live data from your own machines over Tailscale. It only reads. It never changes a
service, a container, or a device.

This repository is a Rust workspace with two versions of the app. Both versions show the same
screen and use the same data code:

- **v1** uses [GPUI](https://github.com/zed-industries/zed/tree/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui),
  the UI framework of the Zed editor.
- **v2** uses [Rune](https://github.com/anomalyco/rune/tree/791ee9669248bcf16fe2672d9c5e6f5d69e714ce),
  a native, GPU-rendered Rust UI framework with its own renderer, text, and layout.

## Repository layout

The workspace has three crates:

| Folder | Crate | Contents |
| --- | --- | --- |
| `status-core/` | `server-status-core` (library) | Configuration, collectors, readings, feed history, freshness rules, text formats, and the Windows display queries. No UI code. |
| `v1/` | `server-status-display-v1` (binary) | GPUI app. See [v1/README.md](v1/README.md). |
| `v2/` | `server-status-display-v2` (binary) | Rune app. See [v2/README.md](v2/README.md). |

The workspace root holds the shared files:

- `Cargo.toml`: workspace members, shared dependency versions, and the Git patches that GPUI and
  Rune need. Cargo applies patches from the workspace root only.
- `Cargo.lock`: one lock file for all crates.
- `rust-toolchain.toml`: the Rust toolchain.
- `config.example.toml`: the example configuration. `status-core` embeds it.
- `assets/fonts/`: IBM Plex Sans and IBM Plex Mono. Both apps bundle these fonts.

## Data sources

The display reads these sources. Each one is optional, except the first two:

| Panel | Source | How the app reads it | Poll interval |
| --- | --- | --- | --- |
| This machine | `sysinfo` crate | Local CPU per core, memory, system disk, network rate, temperature, load, and uptime. | 2 s |
| Tailnet | Tailscale | Runs `tailscale status --json` on the display machine. | 10 s |
| Host panels | [Glances](https://glances.readthedocs.io/) | `GET /api/4/{cpu,percpu,mem,load,sensors,fs,uptime,network}` on each host. | 2 s |
| Services | [Uptime Kuma](https://github.com/louislam/uptime-kuma) | `GET /api/status-page/<slug>` and `/api/status-page/heartbeat/<slug>`. These public status page endpoints need no sign-in. | 30 s |
| Disks | [Scrutiny](https://github.com/AnalogJ/scrutiny) | `GET /api/summary`. SMART result, temperature, and power-on hours per drive. | 5 min |

All requests are HTTP `GET` requests or the read-only Tailscale status command. The app stores no
credentials.

## Configure the display

Both versions read the same configuration file. To configure the display, do these steps:

1. Copy `config.example.toml` to the configuration path for your platform:
   - Windows: `%APPDATA%\server-status-display\config.toml`
   - macOS: `~/Library/Application Support/server-status-display/config.toml`
   - Linux: `~/.config/server-status-display/config.toml`
2. Replace the example URLs with full Tailscale MagicDNS names of your hosts.
3. Set `wallpaper` to an image file, or remove the line for a solid background.

Without a configuration file, the display shows this machine and the tailnet only, on a solid
background. Git ignores `config.local.toml` in the repository root, so you can keep a private
configuration there and pass it with `--config config.local.toml`.

## Run the status display

You need the Rust toolchain in `rust-toolchain.toml`. The first build of each app compiles its UI
framework from Git and takes several minutes.

To run one version, use one of these commands from the repository root:

```sh
cargo run -p server-status-display-v2 --release
cargo run -p server-status-display-v1 --release
```

On Linux, Rune needs the `libxkbcommon` and `wayland` libraries. GPUI needs the libraries in the
Zed Linux build guide.

Both versions take the same command-line options:

| Option | Default | Effect |
| --- | --- | --- |
| `--config <path>` | platform path | Reads this configuration file. |
| `--display <auto, primary, or N>` | `auto` | Selects the monitor. `auto` prefers the physical display on Windows. |
| `--windowed` | off | Opens a normal window instead of fullscreen. |
| `--allow-focus` | off | Windows only. Lets a click give the status window keyboard focus. |

The keyboard shortcuts work only when the status window has keyboard focus:

| Key | Action |
| --- | --- |
| `F11` | Enter or leave fullscreen. |
| `Esc` | Leave fullscreen. The window stays open on the same display. |
| `D` | Move the window to the next display. |
| `R` | Poll all sources now. |
| `Ctrl+Q` (macOS: `Cmd+Q`) | Quit. |

The window also has `Fullscreen` or `Windowed`, `Next display`, `Refresh`, and `Quit` buttons in the
upper-right corner. The buttons and the display label show for 3 seconds after the pointer moves
over the window. On Windows, these buttons are the main controls, because the window does not take
keyboard focus by default.

## Compare the versions

The two versions differ in how the UI framework handles windows and displays:

| Behavior | v1 (GPUI) | v2 (Rune) |
| --- | --- | --- |
| Display list | Live. GPUI reports display changes. | Startup list only. Rune has no display change event. |
| Monitor changes on Windows | Reopens the window on the chosen display after a display event. | A watcher thread reads the Windows display configuration every 2 s and moves the window. |
| Monitor changes on macOS and Linux | Follows GPUI display events (Linux X11: none). | Needs a restart. |
| First open without focus | Windows and macOS. | Windows and macOS. |
| Click without focus on Windows | `WS_EX_NOACTIVATE` through the GPUI window handle. | `WS_EX_NOACTIVATE` through the Rune window handle. |
| Fullscreen on Windows | GPUI borderless fullscreen on the window's display. | winit borderless fullscreen on the window's display. |
| Poll after system wake | Yes, through `App::on_system_wake`. | No wake event. The next scheduled poll updates the data. |
| Screenshots without a display | No. Needs a compositor session. | Yes. Rune's headless `--snap` mode in debug builds. |

Each version README lists its source citations and the items that need a hands-on test.

## Private data

The configuration file and the screen contents name private hosts and services. Keep them out of
Git:

- Git ignores `config.local.toml`. The committed `config.example.toml` uses `example.ts.net` names.
- The repository has no screenshots of real data. Agent screenshots stay in the Git-ignored
  `.agent-screenshots/` folder.
- Git ignores the `local/` folder. Keep the wallpaper file there or in the configuration folder.

## Licenses

The workspace crates use the Apache License 2.0. GPUI uses the Apache License 2.0. Rune uses the MIT
License. The bundled IBM Plex fonts use the SIL Open Font License.
