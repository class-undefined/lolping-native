#!/usr/bin/env python3
"""Install and exercise the shipped Windows application outside the source checkout."""
import argparse
import ctypes
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import wave
import zipfile

from bundle_windows import ARCHITECTURES, RELEASE, verify_machine


def run(*args, **kwargs):
    return subprocess.run(args, check=True, encoding="utf-8", errors="replace", **kwargs)


def ensure_desktop_shell():
    # The Windows ARM runner can log in without starting Explorer. Test the real
    # notification area in a normal desktop session, rather than skipping tray checks.
    find_window = ctypes.WinDLL("user32", use_last_error=True).FindWindowW
    find_window.argtypes = (ctypes.c_wchar_p, ctypes.c_wchar_p)
    find_window.restype = ctypes.c_void_p
    if find_window("Shell_TrayWnd", None):
        print("Explorer taskbar is available")
        return
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise RuntimeError("GUI verification needs a logged-in desktop with Explorer running")
    print("Starting Explorer for the CI desktop session")
    subprocess.Popen([str(pathlib.Path(os.environ["SYSTEMROOT"]) / "explorer.exe")],
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(60):
        if find_window("Shell_TrayWnd", None):
            time.sleep(2)
            print("Explorer taskbar initialized")
            return
        time.sleep(0.5)
    raise RuntimeError("CI desktop has no Explorer taskbar after 30 seconds")


def verify_app(app, machine, working, env):
    for name in ("lolping-native.exe", "hook-helper.exe"):
        verify_machine(app / name, machine)
        imports = run("dumpbin.exe", "/DEPENDENTS", str(app / name), capture_output=True).stdout.upper()
        if "VCRUNTIME" in imports or "MSVCP" in imports:
            raise RuntimeError(f"{name} requires an external Visual C++ runtime")
    result = run(str(app / "lolping-native.exe"), "--list-assets", cwd=working, env=env,
                 capture_output=True, timeout=20)
    assets = result.stdout.splitlines()
    if len(assets) != 29:
        raise RuntimeError(f"Expected 29 assets; got {len(assets)}")
    for name in assets:
        asset = app / "assets" / name
        if not asset.is_file():
            raise RuntimeError(f"Missing installed asset: {name}")
        if asset.suffix == ".wav":
            with wave.open(str(asset), "rb") as sound:
                assert sound.getsampwidth() == 2 and sound.getnframes() > 0
                assert sound.getcomptype() == "NONE"
    # Exercise the real OS hooks as well as the simulated protocol used by the GUI test.
    helper = run(str(app / "hook-helper.exe"), cwd=working, env=env,
                 input='{"type":"shutdown"}\n', capture_output=True, timeout=15)
    events = [json.loads(line) for line in helper.stdout.splitlines()]
    assert any(event.get("type") == "ready" for event in events), events
    assert not any(event.get("type") == "error" for event in events), events
    return assets


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", required=True, choices=ARCHITECTURES)
    args = parser.parse_args()
    ensure_desktop_shell()
    machine = ARCHITECTURES[args.arch][4]
    installers = list(RELEASE.glob(f"*-windows-{args.arch}-setup.exe"))
    archives = list(RELEASE.glob(f"*-windows-{args.arch}.zip"))
    assert len(installers) == len(archives) == 1, "Expected one installer and one portable ZIP"
    installer = installers[0]
    with tempfile.TemporaryDirectory(prefix="lolping-安装验收-") as directory:
        working = pathlib.Path(directory)
        app = working / "已安装应用"
        log = working / "install.log"
        run(str(installer), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART",
            f"/DIR={app}", f"/LOG={log}", timeout=60)
        env = os.environ.copy()
        # Exclude developer tools and isolate user data. No Rust, Python or VC runtime
        # installation is allowed to rescue the installed application's PATH lookup.
        env["PATH"] = str(pathlib.Path(os.environ["SYSTEMROOT"]) / "System32")
        env["APPDATA"] = str(working / "用户配置")
        env["LOCALAPPDATA"] = str(working / "本地配置")
        pathlib.Path(env["APPDATA"]).mkdir()
        pathlib.Path(env["LOCALAPPDATA"]).mkdir()
        assets = verify_app(app, machine, working, env)
        stdout = working / "gui.stdout"
        stderr = working / "gui.stderr"
        with stdout.open("w", encoding="utf-8") as output, stderr.open("w", encoding="utf-8") as errors:
            process = subprocess.Popen([str(app / "lolping-native.exe"), "--smoke-test", "--mute"],
                cwd=working, env=env, stdout=output, stderr=errors)
            try:
                code = process.wait(timeout=90)
            except subprocess.TimeoutExpired:
                subprocess.run(["taskkill", "/F", "/T", "/PID", str(process.pid)], check=False)
                raise RuntimeError("Installed GUI did not finish the smoke test") from None
        output = stderr.read_text(encoding="utf-8", errors="replace")
        logs = RELEASE / "verification-logs"
        logs.mkdir(exist_ok=True)
        (logs / f"windows-{args.arch}.log").write_text(f"GUI exit code: {code}\n{output}", encoding="utf-8")
        print(output)
        if code != 0 or "Smoke test passed:" not in output:
            raise RuntimeError(f"Installed GUI smoke test failed (exit {code})")
        portable = working / "便携应用"
        with zipfile.ZipFile(archives[0]) as archive:
            archive.extractall(portable)
        portable_app = next(portable.iterdir())
        verify_app(portable_app, machine, working, env)
        run(str(app / "unins000.exe"), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", timeout=60)
        assert not (app / "lolping-native.exe").exists(), "Uninstall left the application behind"
    report = {
        "architecture": args.arch, "commit": os.environ.get("GITHUB_SHA"),
        "installer": installer.name, "sha256": hashlib.sha256(installer.read_bytes()).hexdigest(),
        "checks": ["native executable architecture", "static Visual C++ runtime", "Unicode installation path",
                   f"{len(assets)} assets and PCM audio decoding", "real input hook initialization",
                   "installed GUI: overlays, tray, wheel, editor, undo, persistence", "portable ZIP", "uninstall"],
        "audio_output": "muted in CI; PCM files decoded, speaker playback requires an audio device",
    }
    (RELEASE / f"verification-windows-{args.arch}.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"Installation verification passed: Windows {args.arch}")


if __name__ == "__main__":
    main()
