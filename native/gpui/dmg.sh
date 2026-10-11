#!/bin/sh
# Build the native app and package a compressed drag-to-install DMG.
set -eu

usage() {
    printf 'Usage: %s [--skip-build]\n' "$0"
    printf '  --skip-build  Package the existing app without rebuilding.\n'
}

skip_build=false
case "$#" in
    0) ;;
    1)
        case "$1" in
            --skip-build) skip_build=true ;;
            -h|--help) usage; exit 0 ;;
            *) usage >&2; exit 2 ;;
        esac
        ;;
    *) usage >&2; exit 2 ;;
esac

if [ "$(uname -s)" != Darwin ]; then
    printf 'DMG packaging requires macOS.\n' >&2
    exit 1
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
release="$root/release/native"
app="$release/lolPing Native.app"

if [ "$skip_build" = false ]; then
    python3 "$script_dir/bundle.py"
fi
if [ ! -f "$app/Contents/Info.plist" ]; then
    printf 'App bundle not found: %s; run without --skip-build first.\n' "$app" >&2
    exit 1
fi

codesign --verify --deep --strict "$app"
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")
architectures=$(lipo -archs "$app/Contents/MacOS/lolping-native")
case "$architectures" in
    *' '*) architecture=universal ;;
    *) architecture="$architectures" ;;
esac
destination="$release/lolPing-Native-$version-$architecture.dmg"
staging=$(mktemp -d "$release/lolping-dmg-XXXXXX")
trap 'rm -rf "$staging"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

mkdir "$staging/contents"
# Preserve the signed bundle's metadata and resource forks.
ditto "$app" "$staging/contents/lolPing Native.app"
ln -s /Applications "$staging/contents/Applications"
hdiutil create -volname 'lolPing Native' -srcfolder "$staging/contents" \
    -format UDZO -fs HFS+ "$staging/installer.dmg"
hdiutil verify "$staging/installer.dmg"
# Replace the previous image only after verification succeeds.
mv -f "$staging/installer.dmg" "$destination"
printf 'DMG: %s\n' "$destination"
