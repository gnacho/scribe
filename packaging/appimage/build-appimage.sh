#!/usr/bin/env bash
# Builds a Scribe AppImage.
#
# Usage: packaging/appimage/build-appimage.sh <binary> <version>
#
# Bundling is manual on purpose: linuxdeploy-plugin-gtk has no GTK4 support
# (linuxdeploy/linuxdeploy-plugin-gtk#20), so the GTK stack is collected with
# ldd here. The host keeps glibc, X11, Wayland and Mesa/EGL on purpose: those
# must match the session, and bundling them is the usual source of broken
# AppImages. Everything a GTK4 app needs on top of that is bundled.
#
# APPIMAGETOOL can point to an appimagetool binary; otherwise a plain
# `appimagetool` in PATH is used.

set -euo pipefail

BIN="${1:?usage: build-appimage.sh <binary> <version>}"
VERSION="${2:?usage: build-appimage.sh <binary> <version>}"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUILD="$ROOT/target/appimage"
APPDIR="$BUILD/AppDir"
OUT="$BUILD/Scribe-$VERSION-x86_64.AppImage"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" \
	"$APPDIR/usr/share/applications" "$APPDIR/usr/share/metainfo" \
	"$APPDIR/usr/share/glib-2.0/schemas" "$APPDIR/usr/share/locale/es/LC_MESSAGES" \
	"$APPDIR/usr/share/icons/hicolor/scalable/apps" \
	"$APPDIR/usr/share/icons/hicolor/scalable/actions"

install -Dm755 "$BIN" "$APPDIR/usr/bin/scribe"
install -Dm644 "$ROOT/data/io.github.gnacho.Scribe.desktop" "$APPDIR/usr/share/applications/"
install -Dm644 "$ROOT/data/io.github.gnacho.Scribe.metainfo.xml" "$APPDIR/usr/share/metainfo/"
install -Dm644 "$ROOT/data/io.github.gnacho.Scribe.gschema.xml" "$APPDIR/usr/share/glib-2.0/schemas/"
install -Dm644 "$ROOT/data/icons/hicolor/scalable/apps/io.github.gnacho.Scribe.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/"
install -Dm644 "$ROOT/data/icons/hicolor/scalable/actions/"*.svg "$APPDIR/usr/share/icons/hicolor/scalable/actions/"
install -Dm644 "$ROOT/LICENSE" "$APPDIR/usr/share/licenses/scribe/LICENSE"
msgfmt "$ROOT/po/es.po" -o "$APPDIR/usr/share/locale/es/LC_MESSAGES/scribe.mo"

# appimagetool wants the desktop entry and the icon at the AppDir root as well.
install -Dm644 "$ROOT/data/io.github.gnacho.Scribe.desktop" "$APPDIR/io.github.gnacho.Scribe.desktop"
install -Dm644 "$ROOT/data/icons/hicolor/scalable/apps/io.github.gnacho.Scribe.svg" "$APPDIR/io.github.gnacho.Scribe.svg"
ln -sf io.github.gnacho.Scribe.svg "$APPDIR/.DirIcon"

glib-compile-schemas --strict "$APPDIR/usr/share/glib-2.0/schemas"

# Libraries that must come from the host: the C library, the loader, the
# graphics stack of the session and driver-dependent pieces.
keep_host() {
	case "$1" in
	libc.so*|libm.so*|libpthread.so*|libdl.so*|librt.so*|libutil.so*|libcrypt.so*|libresolv.so*|libnss_*|ld-linux*|libgcc_s.so*|libstdc++.so*) return 0 ;;
	libX11*|libxcb*|libXau*|libXdmcp*|libXext*|libXrender*|libXi*|libXfixes*|libXcursor*|libXrandr*|libXdamage*|libXcomposite*|libXkbcommon*) return 0 ;;
	libwayland*|libEGL*|libGL*|libgbm*|libdrm*|libvulkan*) return 0 ;;
	# fontconfig parses /etc/fonts from the host; bundling it mixes an older
	# parser with newer host config and floods the console with warnings.
	libfontconfig*) return 0 ;;
	esac
	return 1
}

copy_deps() {
	local file="$1" dep base
	while read -r dep; do
		[ -n "$dep" ] || continue
		base="$(basename "$dep")"
		keep_host "$base" && continue
		[ -e "$APPDIR/usr/lib/$base" ] && continue
		cp -L "$dep" "$APPDIR/usr/lib/$base"
		copy_deps "$APPDIR/usr/lib/$base"
	done < <(ldd "$file" 2>/dev/null | awk '/=> \// {print $3}')
}

# gdk-pixbuf-query-loaders is not always in PATH: Debian and Ubuntu ship it
# under /usr/lib/<arch>/gdk-pixbuf-2.0/ for the dpkg trigger.
query_loaders() {
	command -v gdk-pixbuf-query-loaders 2>/dev/null && return 0
	local candidate
	for candidate in /usr/lib/*/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders \
		/usr/lib/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders; do
		[ -x "$candidate" ] && { echo "$candidate"; return 0; }
	done
	return 1
}

# App icon and the two symbolic icons are SVG, so the SVG loader is needed at
# runtime; every other loader in the host (PNG/JPEG are built into gdk-pixbuf)
# is bundled as well to keep images working.
LOADERS_SRC="$(find /usr/lib -maxdepth 4 -type d -name loaders -path '*gdk-pixbuf*' | head -1)"
copy_deps "$APPDIR/usr/bin/scribe"
mkdir -p "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders"
if [ -n "$LOADERS_SRC" ]; then
	for loader in "$LOADERS_SRC"/*.so; do
		[ -e "$loader" ] || continue
		cp -L "$loader" "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders/"
		copy_deps "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders/$(basename "$loader")"
	done
	QUERY_LOADERS="$(query_loaders)" || {
		echo "gdk-pixbuf-query-loaders not found" >&2
		exit 1
	}
	"$QUERY_LOADERS" "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders/"*.so \
		| sed "s|$APPDIR|@APPDIR@|g" > "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders/loaders.cache.in"
fi

# Enchant loads its providers (hunspell, aspell) at runtime; the dictionaries
# themselves stay on the host.
ENCHANT_SRC="$(find /usr/lib -maxdepth 4 -type d -name 'enchant-*' | head -1)"
if [ -n "$ENCHANT_SRC" ]; then
	mkdir -p "$APPDIR/usr/lib/enchant-2"
	cp -L "$ENCHANT_SRC"/*.so "$APPDIR/usr/lib/enchant-2/" 2>/dev/null || true
	for provider in "$APPDIR/usr/lib/enchant-2/"*.so; do
		[ -e "$provider" ] || continue
		copy_deps "$provider"
	done
fi

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$HERE/usr/lib:${LD_LIBRARY_PATH:-}"
export GSETTINGS_SCHEMA_DIR="$HERE/usr/share/glib-2.0/schemas"
export XDG_DATA_DIRS="$HERE/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export ENCHANT_MODULE_PATH="$HERE/usr/lib/enchant-2"
if [ -f "$HERE/usr/lib/gdk-pixbuf-2.0/loaders/loaders.cache.in" ]; then
	CACHE="${TMPDIR:-/tmp}/scribe-loaders-$(id -u).cache"
	sed "s|@APPDIR@|$HERE|g" \
		"$HERE/usr/lib/gdk-pixbuf-2.0/loaders/loaders.cache.in" > "$CACHE"
	export GDK_PIXBUF_MODULE_FILE="$CACHE"
fi
exec "$HERE/usr/bin/scribe" "$@"
EOF
chmod +x "$APPDIR/AppRun"

if [ -z "${APPIMAGETOOL:-}" ]; then
	APPIMAGETOOL="$(command -v appimagetool || true)"
fi
[ -n "$APPIMAGETOOL" ] || {
	echo "appimagetool not found: set APPIMAGETOOL or put it in PATH" >&2
	exit 1
}

ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGETOOL" "$APPDIR" "$OUT"
echo "$OUT"
