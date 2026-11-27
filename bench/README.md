# Benchmarks and UI testing

Run these commands from the repository root on Linux.

## Performance benchmarks

Requires Rust, `strip`, `uv`, `kwin_wayland`, `dbus-daemon`, and `spectacle`. Run `cargo fetch --locked` first to cache dependencies.

```sh
out=$(mktemp -d)
uv run bench/status_display_bench.py build --out "$out"
uv run bench/status_display_bench.py run --config config.example.toml --out "$out" \
  --app "current=$out/target/release/server-status-display"
```

Results go to `build-results.json` and `run-results.json` in the output directory. Repeat `--app LABEL=BINARY` to compare builds. Keep screenshots from private configs out of Git.

## Programmatic UI testing

The runner drives the Linux app and checks config reload and the retry button when sleep prevention fails.

Also requires `tesseract`, `pkg-config`, a C++ compiler, and the Qt 6, KWayland, and Wayland client development libraries.

```sh
cargo build --locked
uv run bench/status_display_qa.py --app target/debug/server-status-display
```

Pass `--out <new-directory>` to save screenshots.
