# Server status display

Server status display is a GPUI dashboard for a monitor or a spare computer. It shows the time,
local system metrics, and any remote sources you configure. A fresh profile starts with a solid
background and one card named **This machine**. It makes no network requests for metrics.

## Build and run

Install Rust with rustup. Cargo uses the toolchain in [rust-toolchain.toml](rust-toolchain.toml)
and fetches GPUI at the revision in [Cargo.toml](Cargo.toml).

- On Linux, install the compiler, development libraries, and graphics driver for GPUI. Use the
  pinned [Zed Linux build guide](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/docs/src/development/linux.md)
  for system dependencies. Run inside a Wayland or X11 desktop session.
- On Windows, use a Rust MSVC toolchain, Visual Studio Build Tools with the C++ workload, and the
  Windows SDK. Match the executable architecture to the device.
- On macOS, install Xcode command-line tools and use a Rust toolchain for your Mac. Native macOS
  build and runtime verification are pending.

Run from the repository root:

```sh
cargo run --locked --release -- --windowed
```

Remove `--windowed` to start fullscreen. Pass `--display primary`, `--display auto`, or
`--display 2` to select a monitor. `auto` is the default. On Windows, it prefers a physical
display. The display number starts at 1 in left-to-right order.

| Key | Action when the window has focus |
| --- | --- |
| F11 | Enter or leave fullscreen. |
| Escape | Close the picker or collapse the expanded card. Otherwise, leave fullscreen. |
| D | Move to the next display. |
| R | Refresh all configured sources. |
| Ctrl+Q or Cmd+Q | Quit. |

Open **Wallpaper** in the top-right corner. Move the pointer or lift your finger to show the other
window controls. Tap a host metrics card to see CPU and network graphs in its column. Tap
**Services** to expand the health count into a list of checks. Select **Done** in the expanded
card header to collapse it. Other cards remain interactive.

## Configure sources

You can use local metrics without a configuration file. To rename the local card or add more
sources, copy [config.example.toml](config.example.toml) to the configuration path for your OS.
Edit the file, then restart the app.

| OS | Default configuration path |
| --- | --- |
| Linux | `${XDG_CONFIG_HOME:-~/.config}/server-status-display/config.toml` |
| macOS | `~/Library/Application Support/server-status-display/config.toml` |
| Windows | `%APPDATA%\server-status-display\config.toml` |

To use a different profile, pass `--config /path/to/config.toml`. Create that file first. Use a
separate directory per profile so each has its own saved wallpaper state.

Add one `[[host]]` block per card, in display order. Set `name` to the text you want to see and
`source` to `local`, `glances`, or `tailscale`. If you add remote hosts and want local
metrics, include a `local` block. An empty host list shows one local card.

### Read Glances metrics

1. On the computer you want to monitor, install [Glances v4 with web support](https://glances.readthedocs.io/en/latest/install.html).
   For example, run `uv tool install 'glances[web]'`.
2. Start Glances in web mode. To test on the same computer, run `glances -w -B 127.0.0.1`.
   For a remote computer, bind Glances to its private network address and allow access from the
   display computer.
3. Check that the display computer can read `/api/4/cpu` and `/api/4/mem` on that server.
   For a local setup, run `curl --fail http://127.0.0.1:61208/api/4/cpu`.
4. Add the host block and restart the display. Use the server base URL, without the API path.

```toml
[[host]]
name = "Office server"
source = "glances"
url = "http://127.0.0.1:61208"
```

The app uses read-only HTTP requests. Configure only endpoints that the display can read without
credentials. Use a private network or a restricted reverse proxy for services that expose
system information. Glances works on its own. If you already use Tailscale, you can add
`fallback_device = "office-server"` to show that device's presence when Glances stops responding.

### Open btop in a terminal

On Linux and macOS, expand a local metrics card and select **btop**. Install btop on the machine
you want to inspect. For a remote Glances card, set `btop_ssh` to an OpenSSH alias or
`user@host`. Set up SSH access and verify the host key in OpenSSH before using the button.

The button opens Konsole on Linux, Terminal on macOS, or Windows Terminal on Windows. For
a remote session, the app runs the fixed `btop` command over SSH with strict host-key checking.
OpenSSH handles authentication in the terminal. The dashboard continues polling the configured
metric source independently.

### Add device, service, or disk status

Use the relevant examples in [config.example.toml](config.example.toml). You can enable each
source on its own.

| Source | Configure | Shows |
| --- | --- | --- |
| Tailscale | Install and sign in to Tailscale on the display computer. Confirm `tailscale status --json` works. For a host card, use `source = "tailscale"` and set `device` to the first label of its MagicDNS name. Set `show_devices = true` under `[tailscale]` for the full device panel. | Presence, last seen, OS, and connection type. |
| Uptime Kuma | Set `[uptime_kuma]` with the base `url` and `status_page` slug. The display reads the page and heartbeat endpoints without sign-in. | Monitor status and reported 24-hour uptime. |
| Scrutiny | Set `[scrutiny]` with the web server base `url`. Check that the display can read `/api/summary`. | SMART health, temperature, and power-on hours when reported. |

### Interpret readings

| Reading | What it means |
| --- | --- |
| CPU and memory | Current utilization. The detail panel shows per-core CPU values when the source supplies them and there is room. |
| Disk | Space used on the local system volume or on the filesystem selected from Glances. Scrutiny reports drive health separately. |
| Network | Receive and transmit rates for a selected active interface. The app excludes common virtual interfaces and VPN tunnels. |
| Temperature and load | CPU temperature and the one-minute load average when available. Sensors vary by platform. |
| Uptime | Time since the host started, when reported by the source. |

A dash means the source did not supply that reading. After a failed poll, the app marks the last
reading stale and shows its age. When the reading expires, the app shows the card as offline and
continues polling. You can use the other cards while a source is unavailable.

## Use your wallpapers

1. Open **Wallpaper**, then select **Import image**.
2. Choose a local PNG or JPEG with **Browse**, or enter its path. Enter a display name.
3. Set the panel placement to **Bottom**, **Left**, **Right**, or **Split**. Choose **Glass** for
   translucent panels or **Opaque** for more contrast.
4. Select **Preview wallpaper** to see the image with its settings. Select **Use wallpaper** to save
   and show it. Select **Back** to return to the editor, or **Cancel** to discard the draft.
5. To change a saved wallpaper, return to its card and select **Edit**.

The app saves a reference to the original file. Keep the image at that path, or update its path
with **Edit** if you move it. If the Linux file dialog is unavailable, enter the image path.
The app stores imports, layouts, Favorites, and playback in `status-display-state.toml` next to
the configuration file. Back up this file with your images to preserve your setup.

Select **Favorite** on a card to add it to Favorites. Open **Slideshows** to play Favorites or
create your own list. Add wallpapers, set their order, and choose a duration in days, hours, and
minutes. Favorites starts with a one-hour interval. Select **Play** to start the selected list.
Use a wallpaper preview to switch back to one fixed wallpaper.

Each slide uses its saved layout. Playback waits while you use the picker, read an expanded card, or
hold a press. A slideshow skips missing or unreadable images. If it has no usable image, the
dashboard displays a solid background. On restart, a slideshow starts at its first usable entry.

### Enable presets

Set `preset_wallpapers = "wallpapers"` before the first table in your configuration file. Use a
path relative to that file or an absolute path. Put one PNG or JPEG per style in the folder and
use its style ID as the filename stem. The **Presets** tab lists the local art with its original
style. See [wallpaper filenames, artists, and sources](WALLPAPERS.md).

Keep images, configuration, and state outside Git. For the preset catalog or other third-party
art, retain the artist and source credit with the local copy.

## Move a profile to another device

Exit the app before copying a profile so the export includes your latest saved edits. The
export copies your source settings, images, layouts, Favorites, and slideshows. Copy the whole
exported folder to the other device.

Use the executable from a terminal. On Windows, use `server-status-display.exe`.

```sh
server-status-display --export-profile ./display-backup
server-status-display --import-profile ./display-backup --config ./restored-display/config.toml
server-status-display --config ./restored-display/config.toml
```

For a specific source profile, add `--config /path/to/config.toml` to the export command. To
restore to the default location on a new device, omit `--config` from the import command.
Each transfer needs a new destination directory. It leaves existing profiles intact.

The folder contains your server addresses and artwork. Keep it private. Import profiles you
trust. Review the configuration before opening it on the new device. Set up any device-specific
Tailscale executable paths and SSH aliases there. The export contains no SSH keys, known-host
records, or credentials from other apps.

## Keep the display awake

While the fullscreen window is visible on AC power, the app requests that the OS keep the
display and system awake. It releases the requests on battery or when the window is hidden,
minimized, closed, or windowed. You can still sleep the system manually.

The Windows adapter uses power requests and AC notifications. The macOS adapter uses IOKit
assertions. Linux needs UPower and a desktop portal with Inhibit support. Windows physical
display ranking, AC and battery transitions, touch, and sleep still need hardware QA.

## Source organization

`src/main.rs` starts the app. `src/app.rs` reads the profile and composes the GPUI application.
The project uses one Rust crate.

| Module | Responsibility |
| --- | --- |
| `dashboard` | Window and display lifecycle, view layout, clock, cached feed panels, and graphs. |
| `metrics` | Profile parsing, data sources, polling, and freshness. Each collector has its own file. |
| `wallpaper` | Import editor, local image decoding, layouts, preset catalog, and slideshow state. |
| `platform` | Display selection, native power and display APIs, and the btop terminal launcher. |

OXC's Oxlint binary uses a short entry point, with private implementation modules behind specific
exports. That pattern informed the app composition and feature module exports.
[OXC entry point](https://github.com/oxc-project/oxc/blob/b54b83c555a9ef8ffdb5ec80a708e03b93afd5e2/apps/oxlint/src/main.rs)
· [OXC module exports](https://github.com/oxc-project/oxc/blob/b54b83c555a9ef8ffdb5ec80a708e03b93afd5e2/apps/oxlint/src/lib.rs)

Zed's GPUI platform module selects the native implementation at the platform boundary. The
GPUI window move example shows how to use window-scoped entity callbacks while retaining app
state. Those patterns informed `platform` and the wallpaper state shared across replacement
windows. [Zed platform selection](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_platform/src/gpui_platform.rs)
· [Zed window move example](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/examples/move_entity_between_windows.rs)

## Distribution

Source builds use a locked dependency graph. Before distributing binaries, add per-platform CI
builds and clean-install checks. Package and sign the Windows executable and macOS app bundle.
Notarize the macOS bundle. Document the supported Linux graphics stack. Include the app and
third-party license notices with each artifact. A future Homebrew formula or GitHub download
would need versioned, checksummed artifacts and license review. Preset images stay local.

The app currently uses GPUI without the platform screen-reader adapter. Verify accessibility
before distributing it for general use.

The package declares Apache-2.0. The bundled IBM Plex fonts use the SIL Open Font License.
