# Benchmark

The benchmark measures the build cost, the startup time, and the steady-state resource use of the
status display. Pass two binaries to compare two builds, such as before and after a change.

## Measured values

The benchmark reads these values:

- **Startup:** the app's own `startup: <milestone> after <ms> ms` log lines, and the time at which
  the benchmark read each line from standard error. The milestones are the first view built, the
  first frame presented, and the first poll result of every feed.
- **CPU:** user and system time of the app from `/proc/<pid>/stat`. A separate value counts the
  child processes that the app started and waited for, such as the `tailscale` command.
- **GPU:** the `drm-engine-*` busy time in `/proc/<pid>/fdinfo`, once for each DRM client.
- **Memory:** `VmRSS` from `/proc/<pid>/status` every 5 seconds, and `Pss` from
  `/proc/<pid>/smaps_rollup`.
- **Build:** wall time of `cargo build --release --offline` in an empty target folder, then again
  after a change to the view file.

Each app runs fullscreen, one at a time, in a nested KWin session (`kwin_wayland --virtual`) on its
own D-Bus session bus. The startup runs alternate the order of the binaries.

## Run the benchmark

You need `uv`, `kwin_wayland`, `dbus-daemon`, and `spectacle` on Linux. To run the benchmark, do
these steps from the repository root:

1. Build the app and record the build measurements:

   ```sh
   uv run bench/status_display_bench.py build --out /tmp/status-bench
   ```

2. Measure startup and steady state with your configuration:

   ```sh
   uv run bench/status_display_bench.py run --config config.local.toml --out /tmp/status-bench \
     --app current=/tmp/status-bench/target/release/server-status-display
   ```

The `run` step takes about 5 minutes for each binary. It saves one screenshot of each binary in the
output folder. Keep these screenshots out of Git, because they show your hosts.

## Run dashboard QA

Run the checks against an existing Linux build. In addition to the benchmark
tools, install Tesseract, a C++ compiler, and the Qt 6, KWayland, and Wayland client
development libraries.

```sh
uv run bench/status_display_qa.py --app target/debug/server-status-display
```

The runner checks config reload and failed sleep prevention with a disposable
profile in an isolated KWin session. Use `--out <directory>` to keep screenshots.

## Results

These results show the effect of rendering, polling, and build changes in an earlier revision.
Both binaries ran in one benchmark session on November 23, 2026, with the same configuration.
It had one Glances host card, three presence cards, Uptime Kuma, Scrutiny, and the tailnet panel.

The test machine was an Intel Core i5-9400 with Intel UHD Graphics 630, on CachyOS with KDE Plasma
Wayland. The nested KWin session used one 1920 × 1080 output. The wallpaper file was 3840 × 2160.

### Startup

Startup values are medians of 10 runs, from the app's own log lines:

| Milestone | Before | After |
| --- | --- | --- |
| First view built | 88.7 ms | 81.5 ms |
| First frame presented | 127.0 ms | 119.7 ms |
| All feeds polled | 201.4 ms | 177.1 ms |

The first frame times differ by less than the spread of the runs. The two builds show the first
frame at the same time. All feeds poll sooner, because the Tailscale feeds share one command run.

### Steady state

Steady-state values are medians of 3 runs. Each run measured 60 seconds after a 20-second warm-up:

| Value | Before | After | Change |
| --- | --- | --- | --- |
| App CPU, % of one core | 1.38 | 0.55 | −60 % |
| Child process CPU, % of one core | 0.48 | 0.15 | −69 % |
| GPU busy time, % | 0.90 | 0.35 | −61 % |
| Resident memory (RSS) | 155.4 MiB | 106.3 MiB | −49 MiB |
| Proportional memory (PSS) | 129.2 MiB | 80.1 MiB | −49 MiB |
| Threads | 22 | 21 | −1 |

The benchmark measures all changes together. It does not split the result by change. Expected
causes are:

- **CPU and GPU:** the cached panel views and the redraw timers. Before, the whole view rendered
  and drew once per second.
- **Child process CPU:** one `tailscale status --json` run for each poll round instead of four.
- **Memory:** the wallpaper at 1920 × 1080 instead of 3840 × 2160 in memory and in the GPU atlas.

### Build

Build values come from the `build` subcommand. The "before" values come from an earlier run of
the same subcommand on the same machine:

| Value | Before | After |
| --- | --- | --- |
| Clean release build | 194 s | 196 s |
| Incremental release build | 2.9 s | 58.7 s |
| Release binary | 45.1 MiB | 29.9 MiB |
| Stripped binary | 33.7 MiB | 29.9 MiB |
| Dependency crates | 467 | 466 |

ThinLTO with one codegen unit makes each incremental release build relink and optimize the whole
program. Use `cargo build` for development. The development profile has no LTO.

The benchmark did not measure Windows or macOS.
