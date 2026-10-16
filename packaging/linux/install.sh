#!/bin/sh
# Per-user install of hellocsv (no root needed):  ./install.sh [path/to/hellocsv]   |   ./install.sh --uninstall
# Installs to ~/.local/{bin,share} and registers the .desktop entry + icon + CSV and TSV MIME association.
set -e
here=$(cd "$(dirname "$0")" && pwd)
prefix="${PREFIX:-$HOME/.local}"
apps="$prefix/share/applications"
icons="$prefix/share/icons/hicolor"

refresh() {
    command -v update-desktop-database >/dev/null && update-desktop-database "$apps" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$icons" 2>/dev/null || true
}

if [ "$1" = "--uninstall" ]; then
    rm -f "$prefix/bin/hellocsv" "$apps/hellocsv.desktop"
    for s in 48 128 256; do rm -f "$icons/${s}x${s}/apps/hellocsv.png"; done
    rm -f "$icons/scalable/apps/hellocsv.svg"
    refresh
    echo "hellocsv uninstalled."
    exit 0
fi

bin="${1:-}"
[ -z "$bin" ] && for c in "$here/hellocsv" "$here/../../target/release/hellocsv"; do [ -x "$c" ] && bin="$c" && break; done
[ -x "$bin" ] || { echo "hellocsv binary not found - run 'cargo build --release' or pass its path" >&2; exit 1; }

install -Dm755 "$bin" "$prefix/bin/hellocsv"
install -Dm644 "$here/hellocsv.desktop" "$apps/hellocsv.desktop"
install -Dm644 "$here/hellocsv.svg" "$icons/scalable/apps/hellocsv.svg"
refresh

# make it the default handler for CSV and TSV (the user can undo this in their desktop settings)
if command -v xdg-mime >/dev/null; then
    xdg-mime default hellocsv.desktop text/csv text/tab-separated-values 2>/dev/null || true
fi
echo "hellocsv installed to $prefix/bin/hellocsv"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "note: $prefix/bin is not on your PATH" ;; esac
