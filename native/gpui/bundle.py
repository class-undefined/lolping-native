#!/usr/bin/env python3
"""Build an ad-hoc signed macOS app and a distributable zip without Node/Electron."""
import pathlib
import plistlib
import shutil
import subprocess
import tempfile
import json

ROOT = pathlib.Path(__file__).resolve().parents[2]
RELEASE = ROOT / "release" / "native"


def run(*args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, **kwargs)


def main():
    metadata = json.loads(run("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1",
                              capture_output=True, text=True).stdout)
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "lolping-native")
    run("sh", "native/hook-helper/build.sh")
    run("cargo", "build", "--locked", "--release", "-p", "lolping-native")
    binary = ROOT / "target" / "release" / "lolping-native"
    files = run(str(binary), "--list-assets", capture_output=True, text=True).stdout.splitlines()
    RELEASE.mkdir(parents=True, exist_ok=True)
    # Stage in a temporary directory, so a failed build cannot destroy the last working bundle.
    with tempfile.TemporaryDirectory(prefix="lolping-bundle-", dir=RELEASE) as staging:
        app = pathlib.Path(staging) / "lolPing Native.app"
        macos = app / "Contents" / "MacOS"
        resources = app / "Contents" / "Resources"
        macos.mkdir(parents=True)
        resources.mkdir(parents=True)
        shutil.copy2(binary, macos / "lolping-native")
        shutil.copy2(ROOT / "native/hook-helper/build/hook-helper", macos / "hook-helper")
        for name in files:
            destination = resources / "assets" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / "assets" / name, destination)
        shutil.copy2(ROOT / "LICENSE", resources / "LICENSE")
        shutil.copy2(ROOT / "NOTICE.md", resources / "NOTICE.md")
        # Use Apple's icon tooling instead of shipping an icon-generation runtime.
        iconset = pathlib.Path(staging) / "AppIcon.iconset"
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                suffix = "@2x" if scale == 2 else ""
                run("sips", "-z", str(size * scale), str(size * scale), str(ROOT / "build/icon.png"),
                    "--out", str(iconset / f"icon_{size}x{size}{suffix}.png"), stdout=subprocess.DEVNULL)
        run("iconutil", "-c", "icns", str(iconset), "-o", str(resources / "AppIcon.icns"))
        info = {
            "CFBundleIdentifier": "com.lolping.native", "CFBundleName": "lolPing Native",
            "CFBundleDisplayName": "lolPing Native", "CFBundleExecutable": "lolping-native",
            "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version,
            "CFBundleVersion": "1", "CFBundleIconFile": "AppIcon",
            "LSMinimumSystemVersion": "12.0", "NSHighResolutionCapable": True,
            "NSSupportsAutomaticGraphicsSwitching": True,
            "NSAccessibilityUsageDescription": "lolPing uses keyboard and mouse input to open the ping wheel when you hold its trigger key and drag.",
        }
        (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
        run("codesign", "--force", "--sign", "-", str(macos / "hook-helper"))
        run("codesign", "--force", "--sign", "-", str(app))
        run("codesign", "--verify", "--deep", "--strict", str(app))
        destination = RELEASE / app.name
        if destination.exists():
            shutil.rmtree(destination)
        shutil.move(str(app), destination)
    archive = RELEASE / f"lolPing-Native-{version}-arm64.zip"
    if archive.exists():
        archive.unlink()
    run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(destination), str(archive))
    total = sum(p.stat().st_size for p in destination.rglob("*") if p.is_file())
    print(f"App: {total / 1024**2:.2f} MiB; zip: {archive.stat().st_size / 1024**2:.2f} MiB")
    print(destination)


if __name__ == "__main__":
    main()
