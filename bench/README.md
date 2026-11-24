# Benchmark: GPUI (v1) and Rune (v2)

This benchmark compares the two versions of the status display at feature parity. Both apps show
the same screen from the same data code, so the differences come from the UI frameworks.

## Results

The Rune version starts faster, uses less CPU, and uses less memory. The GPUI version uses about
one third of the GPU time. All differences are small in absolute terms: both apps use less than
1.5 % of one CPU core and less than 3.5 % of the GPU.

These results come from one Linux machine. Windows is the target platform, and Windows was not
measured.

### Startup

Startup is the time from the start of the process to each milestone. The table shows the median of
10 runs for each app. Each run started the app fullscreen at 1920 × 1080:

| Milestone | v1 (GPUI) | v2 (Rune) |
| --- | --- | --- |
| First view built | 85 ms | 50 ms |
| First frame presented | 124 ms | 76 ms |
| All feeds polled | 206 ms | 143 ms |

The first run of each app after a new build was slower. One sample each:

| Milestone, first run | v1 (GPUI) | v2 (Rune) |
| --- | --- | --- |
| First frame presented | 131 ms | 570 ms |

The v1 first-frame time is up to one display refresh (about 17 ms at 60 Hz) late. GPUI runs its
next-frame callback at the start of the frame after the first one. Rune reports the first presented
frame directly through `App::on_frame_drawn`. With 17 ms removed from v1, v2 still presents its
first frame about 30 ms earlier.

"All feeds polled" depends on the network. Both apps poll the same sources with the same
collectors. v1 starts its polls before the window opens, and v2 starts them after the first frame.

### Steady state

Steady state is the resource use of the running app, 20 seconds after start, over a 60-second
window. The table shows the median of 3 runs:

| Measure | v1 (GPUI) | v2 (Rune) |
| --- | --- | --- |
| CPU, % of one core | 1.4 % | 0.9 % |
| GPU busy time | 0.9 % | 3.1 % |
| Resident memory (RSS) | 156 MiB | 103 MiB |
| Proportional memory (PSS) | 129 MiB | 93 MiB |
| Threads | 22 | 17 |
| Frames per second | 1.6 | 1.7 |

Both apps draw about 1.6 frames per second: one for the clock tick and more when poll results
arrive. With the same frame rate, a Rune frame costs about 18 ms of GPU time, and a GPUI frame
costs about 6 ms. The frame rate came from a separate 30-second run with a temporary frame counter.

### Build

| Measure | v1 (GPUI) | v2 (Rune) |
| --- | --- | --- |
| Clean release build | 194 s | 189 s |
| Release rebuild after a view change | 2.9 s | 3.2 s |
| Release binary | 45.1 MiB | 45.6 MiB |
| Stripped release binary | 33.7 MiB | 35.6 MiB |
| Dependency crates | 467 | 335 |

## Recommendation

Keep v1 (GPUI) and remove v2 (Rune). The performance differences are too small to decide the
choice. The requirements decide it:

- **Focus.** GPUI enters fullscreen on Windows with `SWP_NOACTIVATE`. Rune's winit fullscreen code
  does not, so it can take focus from the game. Focus is the main risk of this display.
- **GPU.** The laptop streams a game and encodes video with its GPU. v1 uses about one third of the
  GPU time of v2.
- **Monitor changes.** GPUI reports display changes on Windows and macOS. Rune reads displays once,
  so v2 polls the Windows display configuration and needs a restart on macOS and Linux.

Choose v2 instead if the hands-on Windows test shows that its fullscreen toggle keeps focus in the
game, and if lower memory and faster startup matter more than GPU time.

## Feature parity

Both apps had the same features when this benchmark ran:

- The same configuration, collectors, poll intervals, and freshness rules from `status-core`.
- The same screen: wallpaper, clock, alerts, window buttons, compact host cards, presence cards,
  services, disks, and tailnet panels, with one layout scale and a 1.25 line height.
- The same keyboard shortcuts, command-line options, and display selectors.
- Windowed mode at 80 % of the display size, in the middle of the display.
- A poll of every feed after a system wake. v1 uses GPUI's wake event. v2 detects a wall-clock jump.
- A startup log line for each milestone: `startup: <milestone> after <ms> ms`.

Two differences come from the frameworks and remain:

- v2 cannot follow monitor changes on macOS and Linux, because Rune reports displays only at
  startup.
- v2 follows Windows monitor changes with a 2-second poll instead of an event.

## Test setup

The measurements ran on this machine:

| Item | Value |
| --- | --- |
| CPU | Intel Core i5-9400, 6 cores |
| GPU | Intel UHD Graphics 630 |
| Operating system | CachyOS Linux, kernel 7.3.3 |
| Compositor | Nested KWin 6.8.2 (`kwin_wayland --virtual`), 1920 × 1080 |
| Toolchain | Rust 1.100.0, release profile |
| GPUI | Zed commit `6501df15765d66d6c2a2b66b1eb22434ccef3f86` |
| Rune | Commit `791ee9669248bcf16fe2672d9c5e6f5d69e714ce` |

Each app read the same live configuration: four host cards, Uptime Kuma, Scrutiny, and the tailnet,
over Tailscale. Each app ran fullscreen, one at a time, in a nested KWin session on its own D-Bus
session bus.

The benchmark reads these values:

- **Startup:** the app's own `startup:` log lines, and the time at which the benchmark read each
  line from standard error. Both values agree within 6 ms.
- **CPU:** user and system time from `/proc/<pid>/stat`.
- **GPU:** the `drm-engine-*` busy time in `/proc/<pid>/fdinfo`, once for each DRM client.
- **Memory:** `VmRSS` from `/proc/<pid>/status` every 5 seconds, and `Pss` from
  `/proc/<pid>/smaps_rollup`.
- **Build:** wall time of `cargo build --release --offline` in an empty target folder, then again
  after a change to the view file.

The raw results are in [results/2026-11-23-linux-build.json](results/2026-11-23-linux-build.json)
and [results/2026-11-23-linux-run.json](results/2026-11-23-linux-run.json).

## Run the benchmark

You need `uv`, `kwin_wayland`, `dbus-daemon`, and `spectacle` on Linux. To run the benchmark, do
these steps from the repository root:

1. Build both apps and record the build measurements:

   ```sh
   uv run bench/status_display_bench.py build --out /tmp/status-bench
   ```

2. Measure startup and steady state with your configuration:

   ```sh
   uv run bench/status_display_bench.py run --config config.local.toml --out /tmp/status-bench
   ```

The `run` step takes about 10 minutes. It saves one screenshot of each app in the output folder.
Keep these screenshots out of Git, because they show your hosts.
