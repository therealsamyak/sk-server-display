# Server status display v1 (GPUI)

Version 1 of the server status display uses [GPUI](https://github.com/zed-industries/zed/tree/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui),
the UI framework of the Zed editor, at Zed commit `6501df15765d66d6c2a2b66b1eb22434ccef3f86`. The
data code is in the shared `status-core` crate. The [repository README](../README.md) lists the data
sources, the configuration, the command-line options, and the keyboard shortcuts.

This document is the design of the GPUI version. It states which behavior the GPUI source supports
and which behavior needs a hands-on test.

To run this version, use this command from the repository root:

```sh
cargo run -p server-status-display-v1 --release
```

## Design

### Goals

The status display must do these things:

- Fill the physical laptop monitor and update without user action.
- Never take keyboard input away from the game that Sunshine streams.
- Show clearly when data is old or missing. Old numbers must never look current.
- Stay on the physical monitor when the virtual display appears, disappears, or becomes primary.
- Build from one code base for Windows, macOS, and Linux.

### Screen layout

The screen is a wallpaper with a large clock and translucent data panels. The upper part looks like a
lock screen. The panels look like a terminal system monitor, such as btop. The layout fills the
window and scales from a 1600-pixel reference width. The wireframe shows a 1920 × 1080 display with
one Glances host and one alert:

```text
┌────────────────────────────────────────────────────────────────────────────────────────────────┐
│ wallpaper under a 28 % black layer      [Display 1 of 2 …] [Windowed] [Next display] [Refresh] │
│                                         [Quit]  (only for 3 s after the pointer moves)         │
│  15:42           clock, 128 px            ┌ alert, only when needed ─────────────────────────┐ │
│  Monday 23 November                       │ laptop (local) is not responding.                │ │
│                                           │ Data is 12s old. <last error>                    │ │
│                                           └──────────────────────────────────────────────────┘ │
│ ┌ server ─────────── up 6h 41m ┐ ┌ Services ─────── 14/14 up ┐ ┌ Disks ─────────── 2/3 passed ┐ │
│ │ CPU 10%       38°C load 0.44 │ │ ● Service A          100% │ │ ● nvme0  41°C 477 GiB        │ │
│ │ ▁▂▁▃▂▁▂▅▃▂▁▂▁▂▃▂▁  CPU graph │ │ ● Service B          100% │ │ ● sdb  39°C 932 GiB threshold│ │
│ │ 0 ▬────── 9%  1 ▬────── 9%   │ │ ● Service C         99.8% │ └──────────────────────────────┘ │
│ │ 2 ▬────── 6%  3 ▬────── 5%   │ │ ● Service D         99.9% │ ┌ Tailnet ───────── 5/6 online ┐ │
│ │ Net ↓ 1.6 KiB/s ↑ 2.6 KiB/s  │ │ …                         │ │ ● server  linux this machine │ │
│ │ ▁▁▂▁▁ receive  ▔▔▔ send      │ │                           │ │ ● laptop  windows       idle │ │
│ │ Mem  ██████──── 12.6/31.1 GiB│ │                           │ │ ● tablet  android  2d 7h ago │ │
│ │ Disk ███──────── 135/473 GiB │ │                           │ └──────────────────────────────┘ │
│ └──────────────────────────────┘ └───────────────────────────┘                                  │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The layout rules are:

- The panels fill the lower 64 % of the window. The clock, the date, and the alerts use the rest.
- One host gets a wide first column. Two hosts get one column each. Three or more hosts share one
  column.
- A host panel shows the core grid only when the panel is tall enough. The CPU and network graphs
  shrink first.
- Each panel header shows one fact when the feed is live: uptime, services up, disks passed, or
  devices online. A feed that is not live shows `connecting`, `stale 12s`, or `offline` there, and
  the panel border changes color.
- The CPU graph shows the last 90 samples, 3 minutes at the 2-second poll. Each bar uses the meter
  colors.
- The network graph shows receive rates above the center line in blue and send rates below it in
  purple. Its scale is the highest rate of the shown samples, and at least 64 KiB/s, so idle traffic
  stays flat. The scale value shows in the upper-left corner of the graph.
- Meters and graph bars are green below 75 %, amber from 75 %, and red from 90 %. Temperature is
  amber from 75 °C and red from 85 °C.
- Services sort problems first: down, pending, unknown, maintenance, then up. A service that is not
  up also shows its state and the time of its last heartbeat.
- Disks show a green dot when Scrutiny reports `passed`. A failed disk also shows `SMART failed`,
  `threshold`, or `failed`.
- Tailnet devices show this machine first, then online devices, then offline devices with the time
  since Tailscale last saw them.

The panels use IBM Plex Mono, so digits stay in their columns as values change. The clock and the
date use IBM Plex Sans Light and Regular.

### Wallpaper

Set `wallpaper` in the configuration file to show an image behind the panels. A relative path
starts from the folder of the configuration file. GPUI's `img` element loads the file. The app sets
`ObjectFit::Cover`, so the image fills the window and keeps its aspect ratio
([img.rs#L197-L205](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/elements/img.rs#L197-L205),
[style.rs#L28-L40](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/style.rs#L28-L40)).
GPUI reads JPEG, PNG, WebP, and the other formats in `Img::extensions`.

The repository does not include a wallpaper. The test screenshots use the Windows XP "Bliss" photo
as a placeholder. Microsoft owns that photo, so it stays in the Git-ignored `local/` folder. Replace
it with your own image.

A black layer at 28 % opacity covers the wallpaper, so the white clock stays readable on a bright
image. The panels use a dark color at 78 % opacity.

### Visual cleanup

The screen follows the [unslop-ui](https://github.com/felipebasurto/unslop-ui) rules. These items
were removed from the first version:

- The "Server status" title and the summary line that chained counts with middle dots (U3, U6).
- The overall "ALL LIVE" pill and the "5 of 5 sources live" caption. The alerts already name each
  problem source (U9, U14).
- The `LIVE` badge on each panel. Only a feed that is not live shows its state (U9).
- The source name after each panel title, and the OS and CPU model subtitles (U3, U7).
- The "24 h uptime · last check" caption, and the heartbeat age on services that are up (U5, U14).
- The keyboard shortcut footer and the buttons that always showed. This file lists the shortcuts.
  The buttons show when the pointer moves (U4, U5).
- Uppercase labels such as `CPU ×6`, `MEM`, `TEMP`, and `LOAD` (U1).
- Spaced units in compact values. The panels show `41°C`, `6h 41m`, and `2d 7h ago`.
- The `passed` label on each healthy disk. The green dot already shows it (U14).
- The size and the middle dots in the display label (U6).

The remaining dots and state labels render from feed or row state. Spacing uses steps of 4 design
pixels, with 2 and 6 pixels inside graphs and meters. The heaviest font weight is SemiBold. Nothing animates.

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
| Connecting | No successful poll yet, and the offline limit is not reached. | Blue `connecting` in the panel header. "Waiting for data" in the panel. |
| Live | Last success is newer than the stale limit. | The header fact, such as `14/14 up`. Normal colors. |
| Stale | Last success is between the stale and offline limits. | Amber `stale 12s` and amber border. An amber alert with the data age and the last error. Values and graphs are gray. |
| Offline | Last success is older than the offline limit, or no success before it. | Red `offline`, red border, and a red alert with the retry interval. The panel shows "No data" and the last error. |

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
mode. The window buttons work with the mouse. Use them to leave fullscreen, change display, refresh,
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
`auto` uses the primary display, and the display label shows "primary".

The display label shows the number and the kind of the chosen display, such as
`Display 1 of 2 (built-in panel)`. It shows next to the
window buttons after the pointer moves. Use this label to confirm the choice.

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

- Move the pointer over the window, then click `Windowed`. This works in every mode.
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

The app bundles IBM Plex Sans Light and Regular in `assets/fonts/ibm-plex-sans`, and IBM Plex Mono
Regular and SemiBold in `assets/fonts/ibm-plex-mono`. Both use the SIL Open Font License. GPUI on
Linux maps the system UI font to IBM Plex Sans, but a standalone app does
not have that font
([gpui_linux/src/linux/platform.rs#L132](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/platform.rs#L132)).
The bundled fonts give the same text on every platform.

### Module layout

Each module has one concern. The `status-core` modules are shared with v2:

| File | Concern |
| --- | --- |
| `v1/src/main.rs` | Command-line options, key bindings, and app start. |
| `v1/src/status_board.rs` | GPUI model: the poll loop, the redraw tick, and the wake refresh. |
| `v1/src/status_fonts.rs` | Bundled IBM Plex Sans and IBM Plex Mono font registration. |
| `v1/src/status_view.rs` | GPUI view: wallpaper, clock, panels, graphs, alerts, buttons, and actions. |
| `v1/src/status_window.rs` | Window open, fullscreen state, and monitor change handling. |
| `v1/src/display_select.rs` | Display selectors mapped to GPUI displays. |
| `v1/src/display_windows.rs` | `WS_EX_NOACTIVATE` on the GPUI window handle. |
| `status-core/src/status_config.rs` | TOML configuration and the feed list. |
| `status-core/src/status_collector.rs` | Collector switch for all sources. |
| `status-core/src/collector_local.rs` | This machine through `sysinfo`. |
| `status-core/src/collector_glances.rs` | Glances REST API version 4. |
| `status-core/src/collector_kuma.rs` | Uptime Kuma public status page API. |
| `status-core/src/collector_scrutiny.rs` | Scrutiny summary API. |
| `status-core/src/collector_tailscale.rs` | `tailscale status --json`. |
| `status-core/src/status_model.rs` | Reading types for hosts, services, disks, and devices. |
| `status-core/src/status_feed.rs` | Feed state: latest reading, CPU and network history, and poll errors. |
| `status-core/src/status_freshness.rs` | Freshness limits and feed states. |
| `status-core/src/status_format.rs` | Size, rate, age, uptime, and clock text. |
| `status-core/src/status_display.rs` | Display selectors and the physical display ranking. |
| `status-core/src/display_windows.rs` | Windows display configuration queries and the no-activate style. |

## Verification

### Verified in this version

These checks ran on a Linux machine (KDE Plasma, Wayland) with GPUI at the pinned commit:

- `cargo test --workspace` passes 21 unit tests in `status-core`. The tests cover the
  configuration, the Glances uptime and temperature parsing, the network interface choice, the
  Kuma, Scrutiny, and Tailscale response conversion, the feed history, the freshness rules, the
  formatting, the display selectors, and the physical display ranking. Test inputs use
  `example.ts.net` names.
- `cargo clippy --workspace --all-targets` reports no warnings for Linux.
- `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu` reports no warnings.
  This check compiles the Windows display, focus, `sysinfo`, and Tailscale command code. It does
  not run the code. The [v2 README](../v2/README.md#verified-in-this-version) lists the temporary
  cross-compile setup.

The runs below used the single-crate version before the workspace split (commit `6dd009b`). The
split moved the data code to `status-core` without a change in behavior. The v1 app was not run
again after the split.

- The app ran against the real Glances, Uptime Kuma, Scrutiny, and Tailscale sources over Tailscale,
  in a nested KWin Wayland session with two virtual outputs. A focused app on output 2 stood in for
  the game.
- A Glances entry for an offline tailnet device showed the offline panel and alert after 30 s.
- The wallpaper, clock, CPU and network graphs, and core grid rendered at 1920 × 1080 and
  1280 × 720. A Glances entry for a closed port showed the offline panel and alert. With no other
  window focused, the keyboard focus alert showed.
- The window buttons did not show in these screenshots, because the virtual session does not move
  the pointer. The pointer reveal needs a hands-on check.

### Hands-on verification

These items need a test on real hardware. Windows items come first:

1. **Windows display ranking.** Start a Sunshine stream with the virtual display. Confirm that the
   display label shows the built-in panel. Record the output technology that the virtual display driver
   reports. The ranking treats indirect virtual and unknown outputs as virtual.
2. **Windows primary display swap.** Start and stop a stream that makes the virtual display primary.
   Confirm that the status window stays on, or returns to, the physical panel within about 1 second.
3. **`HMONITOR` stability.** Confirm that GPUI `DisplayId` values match the `HMONITOR` values after a
   display change.
4. **`WS_EX_NOACTIVATE` with GPUI.** Click the status window during a stream. Confirm that the game
   keeps input and that the window buttons show and work. Confirm that GPUI's DirectComposition
   renderer draws normally with this style.
5. **Taskbar overlap.** The status window is not the foreground window, so Windows can draw the
   taskbar over it. Check whether the taskbar covers the bottom of the panels. If it does, use taskbar auto-hide or
   move the taskbar to another display.
6. **Sleep and lid.** Close and open the laptop lid during a stream. Confirm that the display
   recovers and each panel header shows its live fact again.
7. **macOS fullscreen focus.** Confirm whether native fullscreen takes focus from the streamed app.
   If it does, test `toggle_simple_fullscreen`.
8. **Linux X11.** Confirm fullscreen placement with more than one monitor. GPUI X11 sees one display
   for the whole X screen.

## Open questions

- Which wallpaper do you want? The placeholder is the Windows XP "Bliss" photo.
- A host without a metrics server shows only as a tailnet row. Install Glances on that host to add
  a host card.
- Must the app start at sign-in on the Windows laptop? This version adds no startup entry or
  service.
