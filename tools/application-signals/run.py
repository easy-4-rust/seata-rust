"""Deliver real OS events to an owned isolated application process and verify drain."""
from __future__ import annotations
import argparse
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / "docs/phase-2/evidence/application-signals"
TEST = "real_os_signal_drains_native_tm_scope"


def deliver_windows(pid: int, event: int) -> None:
    # This short-lived helper attaches only to the hidden console it was given.
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.FreeConsole.argtypes = []
    kernel.FreeConsole.restype = wintypes.BOOL
    kernel.AttachConsole.argtypes = [wintypes.DWORD]
    kernel.AttachConsole.restype = wintypes.BOOL
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.DWORD)
    kernel.SetConsoleCtrlHandler.argtypes = [callback_type, wintypes.BOOL]
    kernel.SetConsoleCtrlHandler.restype = wintypes.BOOL
    kernel.GenerateConsoleCtrlEvent.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.GenerateConsoleCtrlEvent.restype = wintypes.BOOL
    kernel.FreeConsole()
    if not kernel.AttachConsole(pid):
        raise ctypes.WinError(ctypes.get_last_error())
    # Broadcasting inside the child's private console also reaches this helper.
    handler = callback_type(lambda _: True)
    try:
        if not kernel.SetConsoleCtrlHandler(handler, True):
            raise ctypes.WinError(ctypes.get_last_error())
        if not kernel.GenerateConsoleCtrlEvent(event, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        time.sleep(0.15)
    finally:
        kernel.FreeConsole()


def wait_json(process: subprocess.Popen, path: Path, timeout: float = 15) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            try:
                return json.loads(path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                pass
        code = process.poll()
        if code is not None:
            raise RuntimeError(f"owned child exited {code} before {path.name}")
        time.sleep(0.02)
    raise TimeoutError(f"owned child {process.pid} did not publish {path.name}")


def environment(version: str) -> dict:
    env = dict(os.environ)
    env.update(
        TEMP="D:/Temp", TMP="D:/Temp", PYTHONDONTWRITEBYTECODE="1",
        CARGO_HOME=str(ROOT.parent / ".seata-cargo-cache"),
        CARGO_TARGET_DIR=str(ROOT.parent / (
            ".seata-target-nodebug-185" if version == "1.85" else ".seata-target-nodebug")),
        CARGO_PROFILE_TEST_DEBUG="0", CARGO_INCREMENTAL="0",
    )
    return env


def build(version: str, folder: Path) -> Path:
    command = [
        "cargo", f"+{version}.0", "test", "-p", "seata-tm",
        "--test", "application_signals", "--features", "application_signals",
        "--no-run", "--offline", "--locked", "--message-format=json",
    ]
    with (folder / "build.jsonl").open("wb") as out, (folder / "build.log").open("wb") as err:
        subprocess.run(command, cwd=ROOT, env=environment(version),
                       stdout=out, stderr=err, check=True, timeout=180)
    artifacts = [
        json.loads(line) for line in (folder / "build.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    binaries = [
        Path(row["executable"]) for row in artifacts
        if row.get("reason") == "compiler-artifact" and row.get("executable")
        and row["target"]["name"] == "application_signals"
    ]
    if len(binaries) != 1:
        raise RuntimeError("build did not produce exactly one intended test binary")
    return binaries[0]


def run_case(binary: Path, version: str, event: str, folder: Path) -> dict:
    folder.mkdir()
    env = environment(version)
    env.update(SEATA_SIGNAL_OUT=str(folder), SEATA_EXPECTED_SIGNAL=event)
    options = {}
    if os.name == "nt":
        startup = subprocess.STARTUPINFO()
        startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        options.update(startupinfo=startup, creationflags=subprocess.CREATE_NEW_CONSOLE)
    with (folder / "stdout.log").open("wb") as out, (folder / "stderr.log").open("wb") as err:
        process = subprocess.Popen(
            [str(binary), "--exact", TEST, "--ignored", "--nocapture"],
            cwd=ROOT, env=env, stdout=out, stderr=err, stdin=subprocess.DEVNULL, **options,
        )
        try:
            ready = wait_json(process, folder / "ready.json")
            if ready != {"pid": process.pid, "active": 1, "commit_received": True}:
                raise AssertionError(f"wrong child identity or pre-signal state: {ready}")
            if (folder / "received.json").exists() or (folder / "completed.json").exists():
                raise AssertionError("shutdown completed before external signal delivery")
            if process.poll() is not None:
                raise AssertionError("owned child stopped before signal delivery")
            def deliver():
                if os.name == "nt":
                    event_id = {"interrupt": 0, "console_break": 1}[event]
                    subprocess.run(
                        [sys.executable, "-B", str(Path(__file__).resolve()), "--deliver",
                         str(process.pid), str(event_id)], check=True, timeout=10,
                        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                    )
                else:
                    event_id = {"interrupt": signal.SIGINT, "terminate": signal.SIGTERM}[event]
                    process.send_signal(event_id)
                return event_id
            event_id = deliver()
            received = wait_json(process, folder / "received.json")
            if received != {"signal_matched": True, "admission_closed": True,
                            "active": 1, "owner_open": True}:
                raise AssertionError(f"signal did not preserve the live scope: {received}")
            if process.poll() is not None or (folder / "completed.json").exists():
                raise AssertionError("child exited or completed before Commit release")
            deliver()
            if process.poll() is not None or (folder / "completed.json").exists():
                raise AssertionError("repeated signal killed or prematurely completed the scope")
            (folder / "release").write_text("controller releases original Commit response", encoding="utf-8")
            completed = wait_json(process, folder / "completed.json")
            code = process.wait(timeout=10)
            if code != 0 or completed != {
                "commit_requests": 1, "active": 0, "owner_closed": True, "peer_eof": True,
                "repeated_signal": True
            }:
                raise AssertionError(f"invalid completed shutdown: {code}, {completed}")
            result = dict(platform=sys.platform, toolchain=version, event=event,
                          event_id=int(event_id), deliveries=2, pid=process.pid, ready=ready,
                          received=received, completed=completed, exit_code=code)
            (folder / "verified.json").write_text(
                json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            return result
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=10)


def main() -> None:
    if len(sys.argv) == 4 and sys.argv[1] == "--deliver":
        deliver_windows(int(sys.argv[2]), int(sys.argv[3]))
        return
    parser = argparse.ArgumentParser()
    parser.add_argument("version", choices=["1.99", "1.85"])
    args = parser.parse_args()
    folder = EVIDENCE / f"run-{args.version}-{uuid.uuid4().hex[:8]}"
    folder.mkdir(parents=True)
    print(folder, flush=True)
    binary = build(args.version, folder)
    events = ["interrupt", "console_break"] if os.name == "nt" else ["interrupt", "terminate"]
    results = [run_case(binary, args.version, event, folder / event) for event in events]
    (folder / "verified.json").write_text(
        json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
