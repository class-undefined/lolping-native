#!/usr/bin/env python3
"""Build a self-contained Windows directory, portable ZIP and Inno Setup EXE."""
import os
import pathlib
import shutil
import subprocess
import tempfile
import json

ROOT = pathlib.Path(__file__).resolve().parents[2]
RELEASE = ROOT / "release" / "native"


def run(*args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, **kwargs)


def main():
    if os.name != "nt":
        raise SystemExit("Windows packaging requires Windows, MSVC and Inno Setup 6.")
    metadata = json.loads(run("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1",
                              capture_output=True, text=True).stdout)
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "lolping-native")
    compiler = shutil.which("ISCC.exe")
    if not compiler:
        compiler = str(pathlib.Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)"))
                       / "Inno Setup 6" / "ISCC.exe")
    if not pathlib.Path(compiler).is_file():
        raise SystemExit("Inno Setup 6 not found. Add ISCC.exe to PATH or install Inno Setup 6.")
    run("powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "native/hook-helper/build.ps1")
    run("cargo", "build", "--locked", "--release", "--target", "x86_64-pc-windows-msvc", "-p", "lolping-native")
    binary = ROOT / "target/x86_64-pc-windows-msvc/release/lolping-native.exe"
    files = run(str(binary), "--list-assets", capture_output=True, text=True, encoding="utf-8").stdout.splitlines()
    if not files:
        raise RuntimeError("The application returned an empty asset manifest")
    RELEASE.mkdir(parents=True, exist_ok=True)
    name = f"lolPing-Native-{version}-windows-x64"
    with tempfile.TemporaryDirectory(prefix="lolping-windows-", dir=RELEASE) as temporary:
        staging = pathlib.Path(temporary)
        app = staging / name
        app.mkdir()
        shutil.copy2(binary, app / "lolping-native.exe")
        shutil.copy2(ROOT / "native/hook-helper/build/hook-helper.exe", app / "hook-helper.exe")
        for asset in files:
            relative = pathlib.PurePosixPath(asset)
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError(f"Invalid asset path: {asset}")
            destination = app / "assets" / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / "assets" / relative, destination)
        for legal in ("LICENSE", "NOTICE.md"):
            shutil.copy2(ROOT / legal, app / legal)
        run(compiler, f"/DAppVersion={version}", f"/DSourceDir={app}", f"/DOutputDir={staging}",
            str(ROOT / "native/gpui/windows-installer.iss"))
        archive = pathlib.Path(shutil.make_archive(str(staging / name), "zip", staging, name))
        installer = staging / f"{name}-setup.exe"
        if not installer.is_file():
            raise RuntimeError("Inno Setup did not produce the expected installer")
        for artifact in (archive, installer):
            artifact.replace(RELEASE / artifact.name)
        destination = RELEASE / name
        if destination.exists():
            shutil.rmtree(destination)
        shutil.move(str(app), destination)
    print(f"Installer: {RELEASE / installer.name}")
    print(f"Portable ZIP: {RELEASE / archive.name}")


if __name__ == "__main__":
    main()
