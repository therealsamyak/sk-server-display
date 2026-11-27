# /// script
# requires-python = ">=3.12"
# ///
"""Drive the dashboard with fixture profiles in an isolated KWin session."""

from __future__ import annotations

import argparse
import csv
import os
import shlex
import subprocess
import time
from pathlib import Path
from tempfile import TemporaryDirectory

from status_display_bench import NestedSession, start_app, start_nested_session

WIDTH = 1920
HEIGHT = 1080
RELOAD_ERROR = "could not reload configuration"


def build_qa_input(output: Path) -> None:
    flags = subprocess.check_output(
        ["pkg-config", "--cflags", "--libs", "Qt6Core", "wayland-client"],
        text=True,
    )
    subprocess.run(
        [
            "c++", "-std=c++17", "-Wall", "-Wextra", "-Werror", "-I/usr/include/KWayland",
            str(Path(__file__).with_name("status_display_qa_input.cpp")),
            "-o", str(output), *shlex.split(flags), "-lKWaylandClient",
        ],
        check=True,
    )


def send_qa_input(helper: Path, session: NestedSession, *arguments: str) -> None:
    subprocess.run(
        [str(helper), *arguments], env=session.session_env(), check=True, timeout=5,
    )


def read_qa_frame(session: NestedSession, screenshot: Path) -> list[dict[str, str]]:
    screenshot.unlink(missing_ok=True)
    subprocess.run(
        ["spectacle", "-b", "-n", "-f", "-o", str(screenshot)],
        env=session.session_env(),
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=10,
    )
    words = subprocess.check_output(
        ["tesseract", str(screenshot), "stdout", "--psm", "11", "tsv"],
        text=True,
        stderr=subprocess.DEVNULL,
        timeout=10,
    )
    return [word for word in csv.DictReader(words.splitlines(), delimiter="\t") if word["text"]]


def wait_for_qa_frame(
    session: NestedSession,
    screenshot: Path,
    *expected: str,
    absent: str = "",
) -> list[dict[str, str]]:
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        words = read_qa_frame(session, screenshot)
        text = " ".join(word["text"].lower() for word in words)
        if all(phrase in text for phrase in expected) and (not absent or absent not in text):
            return words
        time.sleep(0.15)
    raise RuntimeError(f"Expected {expected!r} in {screenshot.name}")


def qa_word_left(words: list[dict[str, str]], text: str) -> int:
    return int(next(word for word in words if word["text"].lower() == text)["left"])


def write_qa_profile(config: Path, name: str) -> None:
    config.write_text(f'[[host]]\nname = "{name}"\nsource = "local"\n', encoding="utf-8")


def check_profile_refresh(
    binary: Path, session: NestedSession, helper: Path, home: Path, frames: Path,
) -> None:
    config = home / "server-status-display" / "config.toml"
    config.parent.mkdir(parents=True)
    running = start_app(binary, None, session, env_overrides={"XDG_CONFIG_HOME": str(home)})
    try:
        wait_for_qa_frame(session, frames / "startup.png", "this machine")
        write_qa_profile(config, "Configured display")
        send_qa_input(helper, session, "refresh")
        wait_for_qa_frame(session, frames / "first-refresh.png", "configured display")
        config.unlink()
        send_qa_input(helper, session, "refresh")
        wait_for_qa_frame(
            session, frames / "missing-config.png", "configured display", RELOAD_ERROR,
        )
        write_qa_profile(config, "Updated display")
        send_qa_input(helper, session, "refresh")
        wait_for_qa_frame(
            session, frames / "recovered-config.png", "updated display", absent=RELOAD_ERROR,
        )
    finally:
        running.stop()


def check_unavailable_idle_inhibition(
    binary: Path, session: NestedSession, helper: Path, home: Path, frames: Path,
) -> None:
    running = start_app(
        binary, None, session,
        env_overrides={
            "XDG_CONFIG_HOME": str(home),
            "DBUS_SESSION_BUS_ADDRESS": f"unix:path={home / 'missing-session-bus'}",
        },
    )
    try:
        wait_for_qa_frame(session, frames / "idle-startup.png", "updated display")
        send_qa_input(helper, session, "move", str(WIDTH - 85), "57")
        words = wait_for_qa_frame(session, frames / "idle-control.png", "stay awake")
        label = next(word for word in words if word["text"].lower() == "stay")
        x = int(label["left"]) + int(label["width"]) // 2
        y = int(label["top"]) + int(label["height"]) // 2
        send_qa_input(helper, session, "click", str(x), str(y))
        failed = wait_for_qa_frame(
            session, frames / "idle-request-failed.png", "retry stay awake", absent="allow sleep",
        )
        if qa_word_left(words, "windowed") != qa_word_left(failed, "windowed"):
            raise RuntimeError("The stay awake label change moved the toolbar")
    finally:
        running.stop()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path, help="App binary to test.")
    parser.add_argument("--out", type=Path, help="New directory for screenshot evidence.")
    args = parser.parse_args()
    binary = args.app.resolve(strict=True)
    with TemporaryDirectory(prefix="status-display-qa-") as directory:
        scratch = Path(directory)
        frames = args.out.resolve() if args.out else scratch / "frames"
        frames.mkdir(parents=True)
        helper = scratch / "qa-input"
        build_qa_input(helper)
        session = start_nested_session(f"status-qa-{os.getpid()}", WIDTH, HEIGHT)
        try:
            home = scratch / "config"
            check_profile_refresh(binary, session, helper, home, frames)
            print("PASS: Reload a new, missing, and restored default profile.")
            check_unavailable_idle_inhibition(binary, session, helper, home, frames)
            print("PASS: Show retry in place when the desktop cannot confirm idle inhibition.")
        finally:
            session.stop()


if __name__ == "__main__":
    main()
