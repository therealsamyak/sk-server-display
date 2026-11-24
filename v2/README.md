# Server status display v2 (Rune)

Version 2 of the server status display uses [Rune](https://github.com/anomalyco/rune/tree/791ee9669248bcf16fe2672d9c5e6f5d69e714ce)
at commit `791ee9669248bcf16fe2672d9c5e6f5d69e714ce`. Rune is a native, GPU-rendered Rust UI
framework. It draws with its own renderer and uses a patched winit for windows. The data code is in
the shared `status-core` crate. The [repository README](../README.md) lists the data sources, the
configuration, the command-line options, and the keyboard shortcuts.

This document is the design of the Rune version. It states which behavior the Rune source supports
and which behavior needs a hands-on test. Source links point to the pinned Rune commit.

To run this version, use this command from the repository root:

```sh
cargo run -p server-status-display-v2 --release
```

## Design

### Goals

v2 has the same goals as v1:

- Fill the physical laptop monitor and update without user action.
- Never take keyboard input away from the game that Sunshine streams.
- Show clearly when data is old or missing. Old numbers must never look current.
- Stay on the physical monitor when the virtual display appears, disappears, or becomes primary.
- Build from one code base for Windows, macOS, and Linux.

### Screen layout

v2 draws the same screen as v1. The wireframe, the layout rules, and the visual cleanup rules are in
[v1 screen layout](../v1/README.md#screen-layout) and [v1 visual cleanup](../v1/README.md#visual-cleanup).
The main parts are:

- A wallpaper under a 28 % black layer.
- A 128-pixel clock and the date in the upper-left corner.
- Alerts and the window buttons in the upper-right corner.
- Host, service, disk, and tailnet panels in the lower 64 % of the window.

The Rune view is one function, `status_view(&StatusState, &mut Cx<StatusState>) -> El`. It reads
the size of the root element with `Cx::layout_size` and scales the layout from a 1600-pixel
reference width, as v1 does
([layout_read.rs#L299-L306](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/runtime/layout_read.rs#L299-L306)).

The implementation differs from v1 in these ways:

- **Wallpaper.** `ImageCache::file` decodes the image on Rune's image workers. The view draws it
  with `pixels_fit(.., SurfaceFit::Fill)`, which covers the window and crops the centered overflow
  ([README.md#L1975-L1977](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/README.md#L1975-L1977)).
- **Graphs.** Each CPU and network graph is one vector `Shape` with one filled rectangle for each
  sample. v1 uses one element for each bar.
- **Fonts.** `AppSpec::fonts` bundles IBM Plex Sans and IBM Plex Mono, and `AppSpec::families`
  makes them the sans and mono families
  ([tooling.rs#L26-L31](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/tooling.rs#L26-L31)).
- **Buttons.** The window buttons use `on_press` handlers. The pointer reveal uses
  `on_pointer_move_local` on the root element.

### Refresh, stale, and offline behavior

The freshness rules are the same as in v1. See
[v1 refresh, stale, and offline behavior](../v1/README.md#refresh-stale-and-offline-behavior).
`status-core` holds the rules, so both versions show the same state for the same data.

The poll scheduling uses Rune's threading model:

1. `on_ready` starts one thread for each feed. Each thread polls its collector, then waits for the
   poll interval or a refresh request.
2. The thread sends the result to the UI thread with `AppHandle::update`. Rune runs the closure on
   the UI thread in call order
   ([app.rs#L784-L789](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/app.rs#L784-L789)).
3. `R` and the `Refresh` button send a refresh request to each feed thread. Requests that arrive
   during a poll start one more poll, not several.
4. `AppSpec::poll` ticks the clock once a second. It asks for the next tick with `Tx::wake_at`
   ([tx.rs#L373-L378](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/el/tx.rs#L373-L378)).
   Between ticks and poll results, Rune draws no frames.

A feed thread polls only once at a time, so a slow source does not stack requests. HTTP timeouts are
the same as in v1.

Rune has no system wake event. After sleep, each feed polls again at its next interval. The 2-second
host feeds recover within seconds. The 5-minute disk feed can show `stale` for up to 5 minutes after
a long sleep. Press `R` or click `Refresh` to poll at once.

### Keyboard focus

Keyboard focus is the main risk in this setup. Sunshine sends remote input to the Windows foreground
window. If the status window becomes the foreground window, the game stops receiving input. v2
prevents this in four ways:

1. The app calls `App::open_inactive`. The first open shows the window without focus or app
   activation on macOS and Windows. Elsewhere, the window manager decides
   ([app.rs#L223-L229](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/app.rs#L223-L229)).
2. The app never calls `AppHandle::focus` or `AppHandle::open`. A later `open` or `focus`
   activates the window.
3. On Windows, `App::on_window` runs each time Rune creates the native window
   ([app.rs#L518-L524](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/app.rs#L518-L524)).
   The app gets the `HWND` with `WindowHandle::with_raw_handle`
   ([handle.rs#L184-L188](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/window/handle.rs#L184-L188))
   and adds `WS_EX_NOACTIVATE`. A click then does not make the window the foreground window. Use
   `--allow-focus` to turn this off.
4. `App::on_focus` reports each focus change
   ([app.rs#L275-L280](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/app.rs#L275-L280)).
   When the window has focus, an amber alert says that remote input now goes to the status window.

On Windows, winit's fullscreen code calls `SetWindowPos` without `SWP_NOACTIVATE`
([window.rs#L810-L830](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/vendor/winit/src/platform_impl/windows/window.rs#L810-L830)).
Without that flag, `SetWindowPos` can activate the window. The window enters fullscreen at start and
after each display move, so these two moments can take focus. `WS_EX_NOACTIVATE` can block this
activation, but the source does not prove it. This item needs a hands-on test.

Focus behavior on each platform is:

| Platform | First open | Click | Fullscreen toggle |
| --- | --- | --- | --- |
| Windows | No focus (`open_inactive`). | No focus (`WS_EX_NOACTIVATE`). | Can activate. Needs a test. |
| macOS | No focus (`open_inactive`). | Takes focus. v2 does not use Rune's non-activating panel option. | Native fullscreen. Can take focus. |
| Linux | The window manager decides. | Takes focus. | The compositor or window manager decides. |

On macOS, Rune offers non-activating panels through `MacWindowOptions::nonactivating_panel`
([README.md#L1912-L1916](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/README.md#L1912-L1916)).
v2 does not use this option, because macOS is not the priority platform and a panel needs its own
fullscreen test.

### Display selection

Rune gives the window factory of `rune::run_windows` the list of displays at startup
([tooling.rs#L220](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/tooling.rs#L220)).
Each `Display` has a name, a position and a size in physical pixels, a scale, and a primary flag
([spec.rs#L44-L52](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/window/spec.rs#L44-L52)).
The app sorts the displays from left to right, then top to bottom, and applies the `--display`
selector:

| Selector | Windows | macOS and Linux |
| --- | --- | --- |
| `auto` (default) | The physical display from the `status-core` ranking. | The primary display, or the first display when none is primary. |
| `primary` | The display that winit reports as primary. | Same. |
| `N` | The Nth display. | Same. |

On Windows, `auto` uses the same ranking as v1. `status-core` reads the connector type of each
display with `QueryDisplayConfig` and prefers a built-in panel. It never chooses an indirect virtual
output. The [v1 display selection](../v1/README.md#display-selection) section lists the full
ranking.

The app maps the ranked Windows display to a Rune display by the GDI device name, such as
`\\.\DISPLAY1`. On Windows, winit reports this name as the monitor name
([monitor.rs#L153-L157](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/vendor/winit/src/platform_impl/windows/monitor.rs#L153-L157)).
When the names do not match, the app compares the desktop positions.

The window opens at the chosen display through `WindowOptions::position`, in physical desktop
pixels
([spec.rs#L88-L89](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/window/spec.rs#L88-L89)).
Wayland does not let an app place its window
([app.rs#L722-L725](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/app.rs#L722-L725)).
On Wayland, the compositor chooses the output.

### Fullscreen and exit

The app calls `App::full_screen(true)`, unless you pass `--windowed`. On Windows and Linux, Rune
opens a borderless window over the display that the window opens on
([app.rs#L238-L244](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/app.rs#L238-L244)).
Rune requests winit `Fullscreen::Borderless(None)`, which means the current monitor of the window
([chrome.rs#L127-L133](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/window/chrome.rs#L127-L133)).
On macOS, Rune uses native fullscreen, as the green window button does
([tx.rs#L417-L422](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/el/tx.rs#L417-L422)).

To leave fullscreen, do one of these things:

- Move the pointer over the window, then click `Windowed`. This works in every mode.
- Press `Esc` or `F11` when the window has keyboard focus.

Leaving fullscreen does not quit the app. The window returns to its windowed frame on the same
display. To quit, click `Quit`, press `Ctrl+Q` (macOS: `Cmd+Q`), or close the window.

The app keeps its own fullscreen flag for the button label. If you leave fullscreen through the
operating system, for example with a window manager shortcut, the label can be wrong until the next
toggle.

### Monitor changes

Rune enumerates monitors once, at startup
([README.md#L1911-L1912](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/README.md#L1911-L1912),
[window.rs#L333-L334](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/window.rs#L333-L334)).
It has no display change event. v2 uses a placement watcher thread instead:

1. On Windows, the watcher reads the display configuration with `QueryDisplayConfig` every 2
   seconds. On macOS and Linux, it uses the startup list and runs only when you press `D` or click
   `Next display`.
2. It runs the display selection again.
3. It stops if the chosen display has the same position and size as before.
4. Otherwise, it leaves fullscreen, waits 300 ms, moves the window with `AppHandle::set_position`,
   waits 300 ms, and enters fullscreen again. Winit's borderless fullscreen uses the current monitor
   of the window, so the window fills the new display.
5. It updates the display label.

v2 moves the window instead of opening a new one. The status data does not reset, because the feeds
live in the app state.

A Sunshine stream start can change the display configuration in steps. The watcher acts on each
configuration that it reads, so one stream start can cause more than one move. On macOS and Linux,
restart the app after a monitor change.

### Headless screenshots

Rune debug builds include headless tools: `--snap`, `--size`, `--scale`, `--at`, and `--script`.
With `--realtime`, a headless run waits for the wall clock and runs `on_ready`, so the feed threads
deliver real data
([args.rs#L71-L75](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/tooling/args.rs#L71-L75)).
Release builds get these tools with the `tooling` feature of this crate.

`rune::extra_args` returns the arguments that Rune does not use
([tooling.rs#L135-L146](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/tooling.rs#L135-L146)).
The app parses only these arguments with `clap`, so the two sets of flags do not conflict.

To render the screen at 1920 × 1080 after 30 seconds of live polling, use this command:

```sh
cargo run -p server-status-display-v2 -- --config config.local.toml --windowed \
  --realtime --size 1920x1080 --scale 1 --at 30 --snap status.png
```

Keep these images out of Git when they show real hosts.

### Module layout

Each module has one concern. The `status-core` modules are shared with v1 and listed in
[v1 module layout](../v1/README.md#module-layout).

| File | Concern |
| --- | --- |
| `v2/src/main.rs` | Command-line options, configuration load, window factory, and fonts. |
| `v2/src/status_state.rs` | App state, the clock tick, the control reveal, and the keyboard shortcuts. |
| `v2/src/status_poller.rs` | One poll thread for each feed, and the refresh requests. |
| `v2/src/display_placement.rs` | Display choice, the placement watcher, and window moves. |
| `v2/src/status_view.rs` | Rune view: wallpaper, clock, alerts, buttons, and panels. |
| `v2/src/status_graphs.rs` | Meters, the CPU and network graphs, and the core grid. |
| `v2/src/status_theme.rs` | Colors, text styles, and the layout scale. |

## Verification

### Verified in this version

These checks ran on a Linux machine (KDE Plasma, Wayland) with Rune at the pinned commit:

- `cargo test --workspace` passes 24 unit tests: 21 in `status-core` and 3 in v2. The v2 tests
  cover the display selectors and the windowed position.
- `cargo clippy --workspace --all-targets` reports no warnings for Linux.
- `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu` reports no warnings. This
  check compiles the Windows focus, display, and placement code of v1 and v2. It does not run the
  code. The check ran in a temporary copy of the repository with these changes:
  - TLS turned off in `ureq`, because the `ring` crate needs a MinGW C compiler.
  - `llvm-windres` for the GPUI manifest and `llvm-dlltool` for import libraries.
  - `clang --target=x86_64-w64-mingw32` for the C code of `psm` and `stacker`, with a one-line
    `windows.h` stub. The machine has no MinGW headers.
- Headless `--realtime` runs read the real Glances, Uptime Kuma, Scrutiny, and Tailscale sources over
  Tailscale. The wallpaper, clock, CPU and network graphs, core grid, and all panels rendered at
  1920 × 1080 and 1366 × 768.
- A Glances entry for a closed port showed the red `offline` header, the red border, the error text,
  and the offline alert after 30 seconds.
- A script that gave the window focus and moved the pointer showed the keyboard focus alert, the
  display label, and the window buttons.

### Hands-on verification

These items need a test on real hardware. Windows items come first:

1. **Fullscreen focus on Windows.** Start the app during a stream. Confirm that the fullscreen
   toggle at start does not take focus from the game. Then force a display move. Confirm the same
   for the toggle after the move.
2. **Windows display ranking.** Start a Sunshine stream with the virtual display. Confirm that the
   display label shows the built-in panel.
3. **Windows primary display swap.** Start and stop a stream that makes the virtual display primary.
   Confirm that the status window stays on, or returns to, the physical panel within about 3
   seconds.
4. **Window move timing.** Confirm that 300 ms is enough for winit to leave fullscreen before the
   move. If the window enters fullscreen on the old display, increase `WINDOW_MOVE_STEP_DELAY`.
5. **`WS_EX_NOACTIVATE` with Rune.** Click the status window during a stream. Confirm that the game
   keeps input and that the buttons work. Confirm that Rune's renderer draws normally with this style.
6. **Taskbar overlap.** Check whether the Windows taskbar covers the bottom of the panels.
7. **Sleep and lid.** Close and open the laptop lid during a stream. Confirm that the panels recover
   at their next poll and that the window stays on the physical panel.
8. **macOS fullscreen focus.** Confirm whether native fullscreen takes focus from the streamed app.

## Open questions

- Rune overlays on Windows are topmost tool windows with `WS_EX_NOACTIVATE`
  ([overlay.rs#L18-L20](https://github.com/anomalyco/rune/blob/791ee9669248bcf16fe2672d9c5e6f5d69e714ce/crates/rune/src/overlay.rs#L18-L20)).
  An overlay that covers the physical display can replace the fullscreen toggle and its focus risk.
  Decide after the fullscreen focus test.
- Rune is at an early version and changes often. Update the pinned commit only together with a new
  hands-on test.
