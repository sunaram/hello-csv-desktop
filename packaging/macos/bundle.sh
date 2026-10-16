#!/bin/sh
# Builds "Hello CSV.app" (+ a .dmg) from the release binary. Run on macOS:
#   cargo build --release [--target aarch64-apple-darwin|x86_64-apple-darwin]
#   packaging/macos/bundle.sh [path/to/hellocsv]
# Set SIGN_ID="Developer ID Application: Name (TEAMID)" to code-sign; otherwise it is ad-hoc signed
# (runs locally, but Gatekeeper warns on other Macs until it is signed and notarised).
set -e
here=$(cd "$(dirname "$0")" && pwd)
root="$here/../.."
bin="${1:-$root/target/release/hellocsv}"
[ -x "$bin" ] || { echo "binary not found: $bin" >&2; exit 1; }
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)

out="$here/out"
app="$out/Hello CSV.app"
rm -rf "$out"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/hellocsv"
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"

# placeholder icon: render the SVG to an .icns (needs rsvg-convert or falls back to no icon)
svg="$here/../linux/hellocsv.svg"
if command -v rsvg-convert >/dev/null && command -v iconutil >/dev/null; then
    set_dir="$out/hellocsv.iconset"; mkdir -p "$set_dir"
    for s in 16 32 64 128 256 512; do
        rsvg-convert -w $s -h $s "$svg" -o "$set_dir/icon_${s}x${s}.png"
        rsvg-convert -w $((s*2)) -h $((s*2)) "$svg" -o "$set_dir/icon_${s}x${s}@2x.png"
    done
    iconutil -c icns "$set_dir" -o "$app/Contents/Resources/hellocsv.icns"
    rm -rf "$set_dir"
else
    echo "note: rsvg-convert/iconutil not found - building without an icon (brew install librsvg)"
fi

codesign --force --deep --options runtime --sign "${SIGN_ID:--}" "$app"
hdiutil create -volname "Hello CSV" -srcfolder "$app" -ov -format UDZO "$out/hellocsv-$version.dmg" >/dev/null
echo "Built $app and $out/hellocsv-$version.dmg"
