#!/usr/bin/env python3
"""Install and exercise the shipped Windows application outside the source checkout."""
import argparse
import ctypes
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time
import wave
import zipfile

from bundle_windows import ARCHITECTURES, RELEASE, verify_machine


def run(*args, **kwargs):
    return subprocess.run(args, check=True, encoding="utf-8", errors="replace", **kwargs)


def desktop_accepts_tray_icons():
    """Probe the OS shell independently of Rust, GPUI and the application window."""
    from ctypes import wintypes as w

    class NotifyIconData(ctypes.Structure):
        _fields_ = [("cbSize", w.DWORD), ("hWnd", w.HWND), ("uID", w.UINT),
                    ("uFlags", w.UINT), ("uCallbackMessage", w.UINT), ("hIcon", w.HICON),
                    ("szTip", w.WCHAR * 128), ("dwState", w.DWORD), ("dwStateMask", w.DWORD),
                    ("szInfo", w.WCHAR * 256), ("uVersion", w.UINT), ("szInfoTitle", w.WCHAR * 64),
                    ("dwInfoFlags", w.DWORD), ("guidItem", ctypes.c_ubyte * 16), ("hBalloonIcon", w.HICON)]

    user = ctypes.WinDLL("user32", use_last_error=True)
    shell = ctypes.WinDLL("shell32", use_last_error=True)
    ole = ctypes.WinDLL("ole32")
    user.CreateWindowExW.argtypes = (w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD,
        ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, w.HWND, w.HMENU, w.HINSTANCE, ctypes.c_void_p)
    user.CreateWindowExW.restype = w.HWND
    user.DestroyWindow.argtypes = (w.HWND,)
    user.LoadIconW.argtypes = (w.HINSTANCE, ctypes.c_void_p)
    user.LoadIconW.restype = w.HICON
    shell.Shell_NotifyIconW.argtypes = (w.DWORD, ctypes.POINTER(NotifyIconData))
    shell.Shell_NotifyIconW.restype = w.BOOL
    initialized = ole.OleInitialize(None) >= 0
    hwnd = user.CreateWindowExW(0x80, "STATIC", "lolPing CI shell probe", 0x80000000,
                               0, 0, 0, 0, None, None, None, None)
    data = NotifyIconData()
    data.cbSize = ctypes.sizeof(data)
    data.hWnd, data.uID, data.uFlags = hwnd, 1, 7
    data.uCallbackMessage = 0x8020
    data.hIcon = user.LoadIconW(None, 32512)
    data.szTip = "lolPing CI shell probe"
    added = bool(shell.Shell_NotifyIconW(0, ctypes.byref(data))) if hwnd else False
    print(f"Independent shell tray probe: added={added}, last_error={ctypes.get_last_error()}", flush=True)
    if added:
        shell.Shell_NotifyIconW(2, ctypes.byref(data))
    if hwnd:
        user.DestroyWindow(hwnd)
    if initialized:
        ole.OleUninitialize()
    return added


def ensure_desktop_shell():
    # The Windows ARM runner can log in without starting Explorer. Test the real
    # notification area in a normal desktop session, rather than skipping tray checks.
    find_window = ctypes.WinDLL("user32", use_last_error=True).FindWindowW
    find_window.argtypes = (ctypes.c_wchar_p, ctypes.c_wchar_p)
    find_window.restype = ctypes.c_void_p
    if os.environ.get("GITHUB_ACTIONS") == "true":
        import winreg
        # Only prepare the disposable CI desktop. The installer/application never
        # change a user's notification policies.
        for hive, hive_name in [(winreg.HKEY_CURRENT_USER, "HKCU"), (winreg.HKEY_LOCAL_MACHINE, "HKLM")]:
            for key_path, value_name, enabled_value in [
                (r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer", "NoTrayItemsDisplay", 0),
                (r"Software\Policies\Microsoft\Windows\Explorer", "DisableNotificationCenter", 0),
            ]:
                try:
                    with winreg.OpenKey(hive, key_path, 0, winreg.KEY_READ | winreg.KEY_SET_VALUE) as key:
                        value, _ = winreg.QueryValueEx(key, value_name)
                        print(f"CI shell policy {hive_name}/{value_name}={value}", flush=True)
                        if value != enabled_value:
                            winreg.SetValueEx(key, value_name, 0, winreg.REG_DWORD, enabled_value)
                except FileNotFoundError:
                    print(f"CI shell policy {hive_name}/{value_name}: not set", flush=True)
        run("powershell.exe", "-NoProfile", "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name='explorer.exe'\" | ForEach-Object { "
            "$owner = Invoke-CimMethod -InputObject $_ -MethodName GetOwner; "
            "[pscustomobject]@{ PID=$_.ProcessId; Session=$_.SessionId; User=$owner.User } "
            "} | ConvertTo-Json -Compress")
        print(f"CI job user: {os.environ.get('USERNAME')}", flush=True)
    if find_window("Shell_TrayWnd", None) and desktop_accepts_tray_icons():
        print("Explorer taskbar and notification area are available", flush=True)
        return
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise RuntimeError("GUI verification needs a logged-in desktop with Explorer running")
    print("Restarting the CI desktop shell to initialize its notification area", flush=True)
    subprocess.run(["taskkill", "/F", "/IM", "explorer.exe"], check=False, capture_output=True)
    subprocess.Popen([str(pathlib.Path(os.environ["SYSTEMROOT"]) / "explorer.exe")],
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(60):
        if find_window("Shell_TrayWnd", None) and desktop_accepts_tray_icons():
            print("Explorer taskbar initialized")
            return
        time.sleep(0.5)
    raise RuntimeError("CI desktop notification area cannot register a standalone Win32 icon")


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


def gui_smoke(app, working, env, arch, label):
    logs = RELEASE / "verification-logs"
    logs.mkdir(exist_ok=True)
    stdout = logs / f"windows-{arch}-{label}.stdout"
    stderr = logs / f"windows-{arch}-{label}.log"
    with stdout.open("w", encoding="utf-8") as output, stderr.open("w", encoding="utf-8") as errors:
        process = subprocess.Popen([str(app / "lolping-native.exe"), "--smoke-test", "--mute"],
            cwd=working, env=env, stdout=output, stderr=errors)
        try:
            code = process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            subprocess.run(["taskkill", "/F", "/T", "/PID", str(process.pid)], check=False)
            code = -1
    output = stderr.read_text(encoding="utf-8", errors="replace")
    print(f"GUI probe {label}: exit {code}\n{output}")
    return code == 0 and "Smoke test passed:" in output


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", required=True, choices=ARCHITECTURES)
    parser.add_argument("--desktop-only", action="store_true")
    args = parser.parse_args()
    ensure_desktop_shell()
    if args.desktop_only:
        return
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
        system = pathlib.Path(os.environ["SYSTEMROOT"])
        env["PATH"] = os.pathsep.join([str(system / "System32"), str(system)])
        env["APPDATA"] = str(working / "用户配置")
        pathlib.Path(env["APPDATA"]).mkdir()
        # LOCALAPPDATA belongs to the Windows shell's actual user profile. Replacing
        # it is not a clean app configuration and can break Windows 11 notifications.
        assets = verify_app(app, machine, working, env)
        if not gui_smoke(app, working, env, args.arch, "installed"):
            # These probes diagnose a failure; they never substitute for acceptance
            # of the installed program in a Unicode directory with a minimal PATH.
            gui_smoke(app, working, os.environ.copy(), args.arch, "original-environment")
            with tempfile.TemporaryDirectory(prefix="lolping-ascii-probe-") as probe:
                copied = pathlib.Path(probe) / "app"
                shutil.copytree(app, copied)
                gui_smoke(copied, probe, os.environ.copy(), args.arch, "ascii-path")
            raise RuntimeError("Installed GUI smoke test failed; see environment/path diagnostic probes")
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
