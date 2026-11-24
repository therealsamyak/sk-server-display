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
