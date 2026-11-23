# Server status display

Server status display is a full-screen, continuously updating status board. It runs on the physical
monitor of a Windows laptop while Sunshine streams a game from a separate virtual display to
Moonlight. It uses [GPUI](https://github.com/zed-industries/zed/tree/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui),
the UI framework of the Zed editor. Windows is the priority platform. The same code builds for macOS
and Linux.

The display reads live data from your own machines over Tailscale. It only reads. It never
changes a service, a container, or a device.

This repository holds a working first version and its design. The design states which behavior the
GPUI source supports and which behavior needs a hands-on test.

## Data sources

The display reads these sources. Each one is optional, except the first two:

| Panel | Source | How the app reads it | Poll interval |
| --- | --- | --- | --- |
| This machine | `sysinfo` crate | Local CPU, memory, system disk, temperature, load, and uptime. | 2 s |
| Tailnet | Tailscale | Runs `tailscale status --json` on the display machine. | 10 s |
| Host cards | [Glances](https://glances.readthedocs.io/) | `GET /api/4/{cpu,mem,load,sensors,fs,uptime,quicklook,system}` on each host. | 2 s |
| Services | [Uptime Kuma](https://github.com/louislam/uptime-kuma) | `GET /api/status-page/<slug>` and `/api/status-page/heartbeat/<slug>`. These public status page endpoints need no sign-in. | 30 s |
| Disks | [Scrutiny](https://github.com/AnalogJ/scrutiny) | `GET /api/summary`. SMART result, temperature, and power-on hours per drive. | 5 min |

All requests are HTTP `GET` requests or the read-only Tailscale status command. The app stores no
credentials.

## Run the status display

You need the Rust toolchain in `rust-toolchain.toml`. GPUI comes from Git at Zed commit
`6501df15765d66d6c2a2b66b1eb22434ccef3f86`. The first build compiles GPUI and takes several minutes.

To run the display, do these steps:

1. Copy `config.example.toml` to the configuration path for your platform:
   - Windows: `%APPDATA%\server-status-display\config.toml`
   - macOS: `~/Library/Application Support/server-status-display/config.toml`
   - Linux: `~/.config/server-status-display/config.toml`
2. Replace the example URLs with full Tailscale MagicDNS names of your hosts.
3. Run `cargo run --release`.

Without a configuration file, the display shows this machine and the tailnet only. Git ignores
`config.local.toml` in the repository root, so you can keep a private configuration there and pass
it with `--config config.local.toml`.

The command-line options are:

| Option | Default | Effect |
| --- | --- | --- |
| `--config <path>` | platform path | Reads this configuration file. |
| `--display <auto, primary, or N>` | `auto` | Selects the monitor. See [Display selection](#display-selection). |
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

The header also has `Fullscreen` or `Windowed`, `Next display`, `Refresh`, and `Quit` buttons. On
Windows, these buttons are the main controls, because the window does not take keyboard focus by
default.

## Private data

The configuration file and the screen contents name private hosts and services. Keep them out of
Git:

- Git ignores `config.local.toml`. The committed `config.example.toml` uses `example.ts.net` names.
- The repository has no screenshots of real data. Agent screenshots stay in the Git-ignored
  `.agent-screenshots/` folder.

## Design

### Goals

The status display must do these things:

- Fill the physical laptop monitor and update without user action.
- Never take keyboard input away from the game that Sunshine streams.
- Show clearly when data is old or missing. Old numbers must never look current.
- Stay on the physical monitor when the virtual display appears, disappears, or becomes primary.
- Build from one code base for Windows, macOS, and Linux.

### Screen layout

The layout fills the window and scales from a 1600-pixel reference width. Host cards fill the top
row. Disks and the tailnet fill the bottom row. Services fill the right column. The wireframe shows
a 1920 × 1080 display:

```text
┌──────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                [Windowed] [Next display] [Refresh] [Quit]    │
│ Server status                                 ( ● ALL LIVE )                      14:02:11   │
│ 2 hosts · 14/14 services up · 2/3 disks passed · 5/6 tailnet online   5 of 5 live  Mon 23 Nov│
│ ┌ ● ALERT (only when needed): keyboard focus, or one line per stale or offline source ─────┐ │
│ └──────────────────────────────────────────────────────────────────────────────────────────┘ │
│ ┌ server  Glances ─────── ● LIVE ┐ ┌ This machine  local ─ ● LIVE ┐ ┌ Services  Kuma ● LIVE ┐ │
│ │ OS · CPU model                 │ │ OS · CPU model               │ │ 14 of 14 up            │ │
│ │ CPU ×6 ██░░░░░░░░░░     8 %    │ │ CPU ×8 ███░░░░░░░░   16 %    │ │ ● Service A 100.0 % 53 s│ │
│ │ MEM    █████░░░░░ 13.5/31.1 GiB│ │ MEM    ████░░░  6.2/15.8 GiB │ │ ● Service B  99.8 % 17 s│ │
│ │ DISK   ███░░░░░ 134/473 GiB    │ │ DISK   █████░░  301/476 GiB  │ │ ...                    │ │
│ │ TEMP 53 °C  LOAD 2.88  UP 2 h  │ │ TEMP —  LOAD —  UP 1 d 4 h   │ │                        │ │
│ └────────────────────────────────┘ └──────────────────────────────┘ │                        │ │
│ ┌ Disks · server  Scrutiny ● LIVE┐ ┌ Tailnet  Tailscale ── ● LIVE ┐ │                        │ │
│ │ ● nvme0  model · 477 GiB passed│ │ ● laptop   windows this machine│ │                        │ │
│ │ ● sdb    model · 932 GiB  threshold failed │ ● server linux direct │ │                        │ │
│ └────────────────────────────────┘ │ ● tablet   seen 2 d 3 h ago  │ └────────────────────────┘ │
│ Display 1 of 2 · built-in panel "Built-in Display" · 1920×1080        F11 · Esc · D · R · Ctrl+Q │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

The layout rules are:

- Each panel shows its own source state: `CONNECTING`, `LIVE`, `STALE · 12 s`, or `OFFLINE`.
- The header pill shows the worst source state. The header summary counts hosts, services, disks,
  and tailnet devices.
- Bars are green below 75 %, amber from 75 %, and red from 90 %. Temperature is amber from 75 °C and
  red from 85 °C.
- Services sort problems first: down, pending, unknown, maintenance, then up.
- Disks show Scrutiny's result: `passed`, `SMART failed`, `threshold failed`, or `failed`.
- Tailnet devices show this machine first, then online devices, then offline devices with the last
  seen time.
- The footer holds only information that you do not need at a glance. On Windows, the taskbar can
  cover the bottom of the screen. See [Hands-on verification](#hands-on-verification).

### Refresh, stale, and offline behavior

Each source is a separate feed with its own poll interval. Polls run on background threads. A
1-second tick redraws the clock and the age labels, and starts each poll that is due. Only one poll
per feed runs at a time, so a slow source does not stack requests. HTTP requests time out at 75 % of
the poll interval, from 1.5 s to 10 s.

The freshness limits come from the poll interval of each feed:

| Value | Rule | 2 s feed | 30 s feed | 5 min feed |
| --- | --- | --- | --- | --- |
| Stale after | 3 missed polls | 6 s | 90 s | 15 min |
| Offline after | 15 missed polls, and at least 30 s | 30 s | 7.5 min | 75 min |

Each feed state uses the age of its last successful poll:

| Feed state | Condition | Display |
| --- | --- | --- |
| `CONNECTING` | No successful poll yet, and the offline limit is not reached. | Blue badge. "Waiting for the first reading". |
| `LIVE` | Last success is newer than the stale limit. | Green badge. Normal colors. |
| `STALE` | Last success is between the stale and offline limits. | Amber badge with the data age, and an amber alert with the failed poll count and the last error. Values are gray. |
| `OFFLINE` | Last success is older than the offline limit, or no success before it. | Red badge and red alert. The panel shows "No current data" and the last error. |

Uptime Kuma, Scrutiny, and Tailscale report their own state inside a live feed. A down service, a
failed disk check, or an offline device shows in its row. It does not make the feed stale.

The display polls again immediately after the system wakes from sleep. GPUI reports wake events
through `App::on_system_wake`
([app.rs#L1900-L1909](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/app.rs#L1900-L1909)).

GPUI limits the frame rate of an inactive window to about 30 frames per second by default
([platform.rs#L2734-L2738](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L2734-L2738)).
The status window is inactive while you play. It redraws once per second, so this limit has no
visible effect.

### Keyboard focus

Keyboard focus is the main risk in this setup. Sunshine sends remote keyboard and mouse input to the
Windows foreground window. If the status window becomes the foreground window, the game stops
receiving input. The design prevents this in four ways:

1. The window opens with `WindowOptions::focus = false`. On Windows, GPUI then shows the window with
   `SW_SHOWNOACTIVATE`
   ([gpui_windows/src/window.rs#L580-L585](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L580-L585)).
2. The app never calls `Window::activate_window` or `App::activate`. On Windows,
   `activate_window` sends a synthetic Alt key press with `SendInput` before `SetForegroundWindow`
   ([gpui_windows/src/window.rs#L817-L868](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L817-L868)).
   During a stream, that key press can reach the game.
3. On Windows, the app adds `WS_EX_NOACTIVATE` to the window after GPUI creates it. GPUI has no
   option for this style. The app gets the `HWND` through GPUI's `HasWindowHandle` implementation
   ([window.rs#L8112](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/window.rs#L8112)).
   With this style, a click on the window does not make it the foreground window. Use
   `--allow-focus` to turn this off.
4. When the window has keyboard focus, an amber alert says that remote input now goes to the status
   window. The view observes activation changes with `observe_window_activation`.

The trade-off of `WS_EX_NOACTIVATE` is that the keyboard shortcuts do not work in the default Windows
mode. The header buttons work with the mouse. Use them to leave fullscreen, change display, refresh,
or quit.

GPUI handles focus differently on each platform:

| Platform | `focus: false` at open | Source |
| --- | --- | --- |
| Windows | Supported. Shows with `SW_SHOWNOACTIVATE`. | [window.rs#L580-L585](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L580-L585) |
| macOS | Supported. Uses `orderFront` instead of `makeKeyAndOrderFront`. | [window.rs#L1370-L1375](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L1370-L1375) |
| Linux | Not supported. `WindowParams::focus` is dead code on Linux. The compositor decides. | [platform.rs#L2825-L2829](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L2825-L2829) |

On macOS, simple fullscreen calls `makeKeyAndOrderFront` when it enters or leaves
([gpui_macos/src/window.rs#L638-L646](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L638-L646)).
Native fullscreen moves the window to its own Space
([window.rs#L2031-L2042](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L2031-L2042)).
Both can take focus on macOS. The focus alert shows when this happens.

### Display selection

The `--display` option selects the monitor:

| Selector | Windows | macOS | Linux |
| --- | --- | --- | --- |
| `auto` (default) | The physical display. See the ranking below. | The primary display. | The primary display. Wayland reports none, so the first display. |
| `primary` | The display at the origin `(0, 0)`. | The display with the menu bar. | X11: the default X screen. Wayland: the first display. |
| `N` | The Nth display from left to right, then top to bottom. | Same. | Same. |

Windows `auto` does not use the Windows primary display. A virtual display setup can make the
virtual display primary while a stream runs. GPUI's Windows primary display is the monitor at point
`(0, 0)`
([gpui_windows/src/display.rs#L83-L96](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/display.rs#L83-L96)),
so it follows that change.

GPUI does not report the connector type or whether a display is virtual. The app reads this
information directly from Windows with `QueryDisplayConfig` and `DisplayConfigGetDeviceInfo`. Then
it ranks the active displays:

1. Built-in panel: internal, embedded DisplayPort, or embedded UDI output. The primary display wins a
   tie.
2. Primary external monitor on a physical connector, such as HDMI or DisplayPort.
3. Other external monitor.
4. Indirect wired display, such as a USB display adapter.
5. Never: indirect virtual output or unknown output. Virtual display drivers use these types.

The app maps the chosen Windows display to a GPUI display with the GDI device name, such as
`\\.\DISPLAY1`, and the `HMONITOR` handle. GPUI on Windows uses the `HMONITOR` value as the
`DisplayId`
([gpui_windows/src/display.rs#L79-L81](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/display.rs#L79-L81)).
This mapping is a GPUI implementation detail, not a public contract. If a GPUI update changes it,
`auto` uses the primary display, and the footer label shows "primary".

The footer shows the chosen display, its number, its kind, and its size. Use this label to confirm
the choice.

### Fullscreen and exit

The window opens with `WindowBounds::Fullscreen`. GPUI then calls the platform fullscreen toggle
after it creates the window
([window.rs#L1755-L1759](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/window.rs#L1755-L1759)).
Each platform uses a different fullscreen type:

| Platform | Fullscreen type | Source |
| --- | --- | --- |
| Windows | Borderless window. GPUI removes the frame and sizes the window to the physical bounds of the display. It uses `SWP_NOACTIVATE`, so the toggle does not take focus. | [window.rs#L306-L366](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L306-L366) |
| macOS | Native fullscreen in a separate Space. `toggle_simple_fullscreen` is also available. | [window.rs#L2031-L2042](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L2031-L2042) |
| Linux Wayland | `xdg_toplevel.set_fullscreen(None)`. The compositor selects the output. | [wayland/window.rs#L1984-L1993](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/wayland/window.rs#L1984-L1993) |
| Linux X11 | `_NET_WM_STATE_FULLSCREEN`. The window manager selects the monitor. | [x11/window.rs#L1716-L1725](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/x11/window.rs#L1716-L1725) |

To leave fullscreen, do one of these things:

- Click `Windowed` in the header. This works in every mode.
- Press `Esc` or `F11` when the window has keyboard focus.

Leaving fullscreen does not quit the app. The window becomes a normal window at 80 % of the display
size. To quit, click `Quit`, press `Ctrl+Q` (macOS: `Cmd+Q`), or close the window.

### Monitor changes

The app subscribes to `App::observe_displays`. GPUI reports `Added`, `Removed`, and `Changed` events
([platform.rs#L123-L141](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L123-L141)).
The platform calls can be spurious
([platform.rs#L424-L428](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L424-L428)).

When a display event arrives, the app does these steps:

1. Waits 750 ms for more events. A Sunshine stream start can add a display, change the primary
   display, and change resolution in one burst.
2. Runs the display selection again.
3. Stops if the chosen display ID and bounds are the same as before.
4. Otherwise, reads the fullscreen state of the old window and opens a new window on the chosen
   display with the same state.
5. Closes the old window.

The app reopens the window instead of moving it. On Windows, GPUI stores the display bounds when the
window enters fullscreen
([window.rs#L342-L348](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L342-L348)).
A moved or resized fullscreen window can keep old bounds. A new window always uses the current bounds.
The status data does not reset, because the feeds live outside the window.

On Windows, GPUI receives `WM_DISPLAYCHANGE` on a hidden top-level window, so display events arrive
with no window open
([gpui_windows/src/platform.rs#L213-L217](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/platform.rs#L213-L217)).
On macOS, GPUI uses the screen parameters notification
([gpui_macos/src/platform.rs#L1465-L1487](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/platform.rs#L1465-L1487)).
On Linux, the Wayland backend reports output changes. The X11 backend reports one display for the
whole X screen and has no RandR change handler in this GPUI revision
([x11/display.rs#L14-L36](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/x11/display.rs#L14-L36)).

### Fonts

The app bundles IBM Plex Sans Regular and SemiBold in `assets/fonts/ibm-plex-sans` under the SIL
Open Font License. GPUI on Linux maps the system UI font to IBM Plex Sans, but a standalone app does
not have that font
([gpui_linux/src/linux/platform.rs#L132](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/platform.rs#L132)).
The bundled font gives the same text on every platform.

### Module layout

Each module has one concern:

| File | Concern |
| --- | --- |
| `src/main.rs` | Command-line options, key bindings, and app start. |
| `src/status_config.rs` | TOML configuration and the feed list. |
| `src/status_collector.rs` | Collector switch for all sources. |
| `src/collector_local.rs` | This machine through `sysinfo`. |
| `src/collector_glances.rs` | Glances REST API version 4. |
| `src/collector_kuma.rs` | Uptime Kuma public status page API. |
| `src/collector_scrutiny.rs` | Scrutiny summary API. |
| `src/collector_tailscale.rs` | `tailscale status --json`. |
| `src/status_model.rs` | Reading types for hosts, services, disks, and devices. |
| `src/status_board.rs` | GPUI model: feeds, poll loop, and errors. |
| `src/status_freshness.rs` | Freshness limits and feed states. |
| `src/status_format.rs` | Size, age, uptime, and clock text. |
| `src/status_fonts.rs` | Bundled IBM Plex Sans font registration. |
| `src/status_view.rs` | GPUI view: layout, panels, alerts, buttons, and actions. |
| `src/status_window.rs` | Window open, fullscreen state, and monitor change handling. |
| `src/display_select.rs` | Display selectors and the physical display ranking. |
| `src/display_windows.rs` | Windows display configuration queries and `WS_EX_NOACTIVATE`. |

## Verification

### Verified in this version

These checks ran on a Linux machine (KDE Plasma, Wayland) with GPUI at the pinned commit:

- `cargo test` passes 17 unit tests. The tests cover the configuration, the Glances uptime and
  temperature parsing, the Kuma, Scrutiny, and Tailscale response conversion, the freshness rules,
  the formatting, the display selectors, and the physical display ranking. Test inputs use
  `example.ts.net` names.
- `cargo clippy --all-targets` reports no warnings for Linux.
- `cargo clippy --all-targets --target x86_64-pc-windows-gnu` reports no warnings. This check
  compiles the Windows display, focus, `sysinfo`, and Tailscale command code. It does not run the
  code. The check ran in a temporary copy with TLS turned off in `ureq`, because the `ring` crate
  needs a MinGW C compiler. It used `llvm-windres` for the GPUI manifest.
- The app ran against the real Glances, Uptime Kuma, Scrutiny, and Tailscale sources over Tailscale,
  in a nested KWin Wayland session with two virtual outputs. A focused app on output 2 stood in for
  the game.
- A Glances entry for an offline tailnet device showed the `OFFLINE` panel and alert after 30 s.

### Hands-on verification

These items need a test on real hardware. Windows items come first:

1. **Windows display ranking.** Start a Sunshine stream with the virtual display. Confirm that the
   footer shows the built-in panel. Record the output technology that the virtual display driver
   reports. The ranking treats indirect virtual and unknown outputs as virtual.
2. **Windows primary display swap.** Start and stop a stream that makes the virtual display primary.
   Confirm that the status window stays on, or returns to, the physical panel within about 1 second.
3. **`HMONITOR` stability.** Confirm that GPUI `DisplayId` values match the `HMONITOR` values after a
   display change.
4. **`WS_EX_NOACTIVATE` with GPUI.** Click the status window during a stream. Confirm that the game
   keeps input and that the header buttons still work. Confirm that GPUI's DirectComposition
   renderer draws normally with this style.
5. **Taskbar overlap.** The status window is not the foreground window, so Windows can draw the
   taskbar over it. Check whether the taskbar covers the footer. If it does, use taskbar auto-hide or
   move the taskbar to another display.
6. **Sleep and lid.** Close and open the laptop lid during a stream. Confirm that the display
   recovers and the source state returns to `LIVE`.
7. **macOS fullscreen focus.** Confirm whether native fullscreen takes focus from the streamed app.
   If it does, test `toggle_simple_fullscreen`.
8. **Linux X11.** Confirm fullscreen placement with more than one monitor. GPUI X11 sees one display
   for the whole X screen.

## Open questions

- A host without a metrics server shows only as a tailnet row. Install Glances on that host to add
  a host card.
- Must the app start at sign-in on the Windows laptop? This version adds no startup entry or
  service.
