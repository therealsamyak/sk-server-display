# /// script
# requires-python = ">=3.12"
# ///
"""Status display benchmark: build cost, startup time, and steady-state use.

Subcommands:

- `build`: times a clean release build and an incremental rebuild of the
  app, and reports the binary size and the dependency count.
- `run`: starts one or more app binaries in a nested KWin Wayland session,
  times startup, and measures steady-state CPU, GPU, and memory use. Pass
  two binaries to compare two builds, such as before and after a change.

Run from the repository root:

    uv run bench/status_display_bench.py build --out /tmp/status-bench
    uv run bench/status_display_bench.py run --config config.local.toml --out /tmp/status-bench \
        --app current=/tmp/status-bench/target/release/server-status-display

The `run` subcommand reads Linux `/proc` data and needs `kwin_wayland`.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import signal
import statistics
import subprocess
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CLOCK_TICKS = os.sysconf("SC_CLK_TCK")
MILESTONE_RE = re.compile(r"startup: (.+) after ([0-9.]+) ms")


PACKAGE = "server-status-display"
VIEW_FILE = "src/status_view.rs"


@dataclass(frozen=True)
class BenchApp:
    """One app binary under test, with a label for the results."""

    key: str
    binary: Path


def cargo_env(target_dir: Path) -> dict[str, str]:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    env["PATH"] = f"{Path.home() / '.cargo/bin'}:{env.get('PATH', '')}"
    return env


def timed_cargo_build(target_dir: Path) -> float:
    started = time.monotonic()
    subprocess.run(
        ["cargo", "build", "--release", "--offline"],
        cwd=REPO_ROOT,
        env=cargo_env(target_dir),
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return time.monotonic() - started


def count_dependencies() -> int:
    output = subprocess.run(
        ["cargo", "tree", "--offline", "-e", "normal", "--prefix", "none"],
        cwd=REPO_ROOT,
        env=cargo_env(REPO_ROOT / "target"),
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    crates = {line.split(" (")[0].removesuffix(" (*)") for line in output.splitlines() if line.strip()}
    return len(crates) - 1


def run_build_bench(out: Path) -> dict:
    """Clean and incremental release builds, binary size, and dependency count."""
    target_dir = out / "target"
    shutil.rmtree(target_dir, ignore_errors=True)
    print("clean release build", flush=True)
    clean_seconds = timed_cargo_build(target_dir)
    os.utime(REPO_ROOT / VIEW_FILE)
    print("incremental release build", flush=True)
    incremental_seconds = timed_cargo_build(target_dir)
    binary = target_dir / "release" / PACKAGE
    stripped = out / f"{PACKAGE}.stripped"
    shutil.copyfile(binary, stripped)
    subprocess.run(["strip", str(stripped)], check=True)
    results = {
        "clean_build_seconds": round(clean_seconds, 1),
        "incremental_build_seconds": round(incremental_seconds, 1),
        "binary_bytes": binary.stat().st_size,
        "stripped_binary_bytes": stripped.stat().st_size,
        "dependency_crates": count_dependencies(),
    }
    print(json.dumps(results), flush=True)
    return results


@dataclass
class AppProcess:
    """A running app with its stderr lines and their arrival times."""

    process: subprocess.Popen
    started: float
    lines: list[tuple[float, str]] = field(default_factory=list)

    def read_stderr(self) -> None:
        assert self.process.stderr is not None
        for line in self.process.stderr:
            self.lines.append(((time.monotonic() - self.started) * 1000.0, line.rstrip()))

    def milestones(self) -> dict[str, dict[str, float]]:
        found = {}
        for arrived_ms, line in self.lines:
            match = MILESTONE_RE.search(line)
            if match and match.group(1) not in found:
                found[match.group(1)] = {"in_app_ms": float(match.group(2)), "observed_ms": round(arrived_ms, 1)}
        return found

    def stop(self) -> None:
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGTERM)
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


def start_app(binary: Path, config: Path, session: NestedSession) -> AppProcess:
    env = session.session_env()
    env["RUST_LOG"] = "info"
    process = subprocess.Popen(
        [str(binary), "--config", str(config)],
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    app = AppProcess(process, time.monotonic())
    threading.Thread(target=app.read_stderr, daemon=True).start()
    return app


@dataclass
class NestedSession:
    """A nested KWin session on its own D-Bus session bus."""

    socket: str
    bus_address: str
    dbus: subprocess.Popen
    kwin: subprocess.Popen

    def session_env(self) -> dict[str, str]:
        env = dict(os.environ)
        env.pop("DISPLAY", None)
        env["WAYLAND_DISPLAY"] = self.socket
        env["DBUS_SESSION_BUS_ADDRESS"] = self.bus_address
        return env

    def stop(self) -> None:
        self.kwin.terminate()
        try:
            self.kwin.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.kwin.kill()
        # KWin starts helper services on this bus. Stop them with the bus.
        for proc in Path("/proc").iterdir():
            if not proc.name.isdigit() or int(proc.name) == os.getpid():
                continue
            try:
                environ = (proc / "environ").read_bytes()
            except OSError:
                continue
            if f"DBUS_SESSION_BUS_ADDRESS={self.bus_address}".encode() in environ:
                try:
                    os.kill(int(proc.name), signal.SIGTERM)
                except ProcessLookupError:
                    pass
        self.dbus.terminate()
        self.dbus.wait(timeout=10)


def start_nested_session(socket: str, width: int, height: int) -> NestedSession:
    runtime_dir = Path(os.environ["XDG_RUNTIME_DIR"])
    (runtime_dir / socket).unlink(missing_ok=True)
    dbus = subprocess.Popen(
        ["dbus-daemon", "--session", "--nofork", "--print-address=1"],
        stdout=subprocess.PIPE,
        text=True,
    )
    assert dbus.stdout is not None
    bus_address = dbus.stdout.readline().strip()
    env = dict(os.environ)
    env.pop("DISPLAY", None)
    env["DBUS_SESSION_BUS_ADDRESS"] = bus_address
    kwin = subprocess.Popen(
        ["kwin_wayland", "--virtual", "--no-lockscreen", "--socket", socket,
         "--width", str(width), "--height", str(height)],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    session = NestedSession(socket, bus_address, dbus, kwin)
    for _ in range(100):
        if (runtime_dir / socket).exists():
            time.sleep(1.0)
            return session
        time.sleep(0.1)
    session.stop()
    raise RuntimeError("nested KWin did not start")


def cpu_seconds(pid: int) -> tuple[float, float]:
    """CPU time of the process, and of its finished child processes."""
    stat = Path(f"/proc/{pid}/stat").read_text()
    fields = stat[stat.rindex(")") + 2 :].split()
    own = (int(fields[11]) + int(fields[12])) / CLOCK_TICKS
    children = (int(fields[13]) + int(fields[14])) / CLOCK_TICKS
    return own, children


def gpu_busy_ns(pid: int) -> int:
    """GPU engine time of the process from DRM fdinfo, once per DRM client."""
    clients: dict[str, int] = {}
    for fdinfo in Path(f"/proc/{pid}/fdinfo").iterdir():
        try:
            text = fdinfo.read_text()
        except OSError:
            continue
        client = re.search(r"^drm-client-id:\s*(\d+)", text, re.M)
        if not client:
            continue
        busy = sum(int(value) for value in re.findall(r"^drm-engine-[\w-]+:\s*(\d+) ns", text, re.M))
        clients[client.group(1)] = busy
    return sum(clients.values())


def memory_kib(pid: int) -> dict[str, int]:
    status = Path(f"/proc/{pid}/status").read_text()
    rollup = Path(f"/proc/{pid}/smaps_rollup").read_text()
    value = lambda text, key: int(re.search(rf"^{key}:\s+(\d+) kB", text, re.M).group(1))
    return {
        "rss_kib": value(status, "VmRSS"),
        "pss_kib": value(rollup, "Pss"),
        "threads": int(re.search(r"^Threads:\s+(\d+)", status, re.M).group(1)),
    }


def measure_startup(binary: Path, config: Path, session: NestedSession, timeout: float) -> dict:
    running = start_app(binary, config, session)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline and "all feeds polled" not in running.milestones():
        if running.process.poll() is not None:
            break
        time.sleep(0.02)
    running.stop()
    return running.milestones()


def measure_steady(binary: Path, config: Path, session: NestedSession, warmup: float,
                   window: float, screenshot: Path | None) -> dict:
    running = start_app(binary, config, session)
    pid = running.process.pid
    time.sleep(warmup)
    cpu_start, gpu_start, wall_start = cpu_seconds(pid), gpu_busy_ns(pid), time.monotonic()
    samples = []
    while time.monotonic() - wall_start < window:
        time.sleep(5.0)
        samples.append(memory_kib(pid))
    wall = time.monotonic() - wall_start
    cpu_end = cpu_seconds(pid)
    cpu_percent = (cpu_end[0] - cpu_start[0]) / wall * 100.0
    child_cpu_percent = (cpu_end[1] - cpu_start[1]) / wall * 100.0
    gpu_percent = (gpu_busy_ns(pid) - gpu_start) / 1e9 / wall * 100.0
    if screenshot is not None:
        subprocess.run(["spectacle", "-b", "-n", "-f", "-o", str(screenshot)], env=session.session_env(),
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
    running.stop()
    return {
        "cpu_percent": round(cpu_percent, 2),
        "child_cpu_percent": round(child_cpu_percent, 2),
        "gpu_percent": round(gpu_percent, 3),
        "rss_mib_mean": round(statistics.mean(s["rss_kib"] for s in samples) / 1024, 1),
        "rss_mib_max": round(max(s["rss_kib"] for s in samples) / 1024, 1),
        "pss_mib_mean": round(statistics.mean(s["pss_kib"] for s in samples) / 1024, 1),
        "threads": samples[-1]["threads"],
    }


def summarize(values: list[float]) -> dict[str, float]:
    values = sorted(values)
    return {
        "median": round(statistics.median(values), 1),
        "min": round(values[0], 1),
        "max": round(values[-1], 1),
        "n": len(values),
    }


def run_runtime_bench(args: argparse.Namespace) -> dict:
    """Startup times and steady-state resource use in a nested KWin session."""
    session = start_nested_session("status-bench", args.width, args.height)
    apps = args.apps
    results: dict = {app.key: {"startup_runs": [], "steady_runs": []} for app in apps}
    try:
        for run in range(args.startup_runs):
            # Alternate the order, so no app always starts second.
            order = apps if run % 2 == 0 else list(reversed(apps))
            for app in order:
                milestones = measure_startup(app.binary, args.config, session, 30.0)
                results[app.key]["startup_runs"].append(milestones)
                print(f"startup {app.key} run {run + 1}: {json.dumps(milestones)}", flush=True)
                time.sleep(1.0)
        for run in range(args.steady_runs):
            for app in apps:
                shot = args.out / f"{app.key}-steady.png" if run == 0 else None
                steady = measure_steady(app.binary, args.config, session,
                                        args.warmup, args.window, shot)
                results[app.key]["steady_runs"].append(steady)
                print(f"steady {app.key} run {run + 1}: {json.dumps(steady)}", flush=True)
                time.sleep(2.0)
    finally:
        session.stop()
    for app in apps:
        runs = results[app.key]["startup_runs"]
        summary = {}
        for milestone in ["first view built", "first frame presented", "all feeds polled"]:
            for kind in ["in_app_ms", "observed_ms"]:
                values = [run[milestone][kind] for run in runs if milestone in run]
                if values:
                    summary[f"{milestone} {kind}"] = summarize(values)
        steady = results[app.key]["steady_runs"]
        for key in steady[0]:
            summary[f"steady {key}"] = summarize([run[key] for run in steady])
        results[app.key]["summary"] = summary
    return results


def parse_bench_app(value: str) -> BenchApp:
    key, _, binary = value.partition("=")
    if not key or not binary:
        raise argparse.ArgumentTypeError("use LABEL=BINARY")
    return BenchApp(key, Path(binary))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    build = sub.add_parser("build", help="Time release builds and report binary sizes.")
    build.add_argument("--out", type=Path, required=True)
    run = sub.add_parser("run", help="Time startup and measure steady-state use.")
    run.add_argument("--config", type=Path, required=True)
    run.add_argument("--out", type=Path, required=True)
    run.add_argument("--app", dest="apps", action="append", required=True, type=parse_bench_app,
                     help="LABEL=BINARY. Repeat to compare builds.")
    run.add_argument("--width", type=int, default=1920)
    run.add_argument("--height", type=int, default=1080)
    run.add_argument("--startup-runs", type=int, default=10)
    run.add_argument("--steady-runs", type=int, default=3)
    run.add_argument("--warmup", type=float, default=20.0)
    run.add_argument("--window", type=float, default=60.0)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    if args.command == "build":
        results = run_build_bench(args.out)
        (args.out / "build-results.json").write_text(json.dumps(results, indent=2))
    else:
        results = run_runtime_bench(args)
        (args.out / "run-results.json").write_text(json.dumps(results, indent=2))
    summary = results if args.command == "build" else {key: value["summary"] for key, value in results.items()}
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
