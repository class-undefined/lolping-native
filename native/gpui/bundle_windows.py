#!/usr/bin/env python3
"""Build a self-contained Windows directory, portable ZIP and Inno Setup EXE."""
import os
import pathlib
import shutil
import subprocess
import tempfile
import json
import argparse
import struct

ROOT = pathlib.Path(__file__).resolve().parents[2]
RELEASE = ROOT / "release" / "native"
ARCHITECTURES = {
    "x86_64": ("x86_64-pc-windows-msvc", "x64", "x64compatible", "10.0.17763", 0x8664),
    "aarch64": ("aarch64-pc-windows-msvc", "arm64", "arm64", "10.0.22000", 0xAA64),
}


def verify_machine(path, expected):
    with pathlib.Path(path).open("rb") as binary:
        if binary.read(2) != b"MZ":
            raise ValueError(f"Not a Windows executable: {path}")
        binary.seek(0x3C)
        offset = struct.unpack("<I", binary.read(4))[0]
        binary.seek(offset)
        if binary.read(4) != b"PE\0\0":
            raise ValueError(f"Invalid PE header: {path}")
        machine = struct.unpack("<H", binary.read(2))[0]
    if machine != expected:
        raise ValueError(f"Wrong architecture in {path}: {machine:#x}, expected {expected:#x}")


def run(*args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=ARCHITECTURES, default="x86_64")
    args = parser.parse_args()
    target, msvc, installer_arch, minimum_windows, machine = ARCHITECTURES[args.arch]
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
    run("powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "native/hook-helper/build.ps1", "-Architecture", msvc)
    run("cargo", "build", "--locked", "--release", "--target", target, "-p", "lolping-native")
    binary = ROOT / "target" / target / "release/lolping-native.exe"
    helper = ROOT / "native/hook-helper/build/hook-helper.exe"
    verify_machine(binary, machine)
    verify_machine(helper, machine)
    files = run(str(binary), "--list-assets", capture_output=True, text=True, encoding="utf-8").stdout.splitlines()
    if not files:
        raise RuntimeError("The application returned an empty asset manifest")
    RELEASE.mkdir(parents=True, exist_ok=True)
    name = f"lolPing-Native-{version}-windows-{args.arch}"
    with tempfile.TemporaryDirectory(prefix="lolping-windows-", dir=RELEASE) as temporary:
        staging = pathlib.Path(temporary)
        app = staging / name
        app.mkdir()
        shutil.copy2(binary, app / "lolping-native.exe")
        shutil.copy2(helper, app / "hook-helper.exe")
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
            f"/DAppArchitecture={args.arch}", f"/DInstallerArchitecture={installer_arch}",
            f"/DMinimumWindowsVersion={minimum_windows}",
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
