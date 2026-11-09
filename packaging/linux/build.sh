#!/usr/bin/env bash
# Packs a Linux release build into dist/linux:
#
#   cerno_<version>_amd64.deb        cargo-deb; depends on the system's libheif and GStreamer
#   cerno_<version>_x86_64.AppImage  portable; bundles libheif and its libde265 plugin, uses
#                                    the system's GStreamer for videos
#   cerno_<version>_x86_64.AppImage.zsync
#                                    the AppImage's block checksums: AppImageUpdate fetches
#                                    only the changed parts of a newer release
#
# Run after `cargo build --release --features heic,video`, on Ubuntu 24.04: it is the
# oldest base with libheif >= 1.17, and the AppImage runs on distributions with
# its glibc (2.39) or newer.
# Needs: cargo-deb, patchelf, libheif-plugin-libde265, curl, zsync (zsyncmake).
set -euo pipefail
cd "$(dirname "$0")/../.."

bin=target/release/cerno
if [[ ! -x $bin ]]; then
    echo "$bin is missing - run: cargo build --release --features heic,video" >&2
    exit 1
fi
if ! ldd "$bin" | grep -q 'libheif\.so'; then
    echo "$bin was built without --features heic" >&2
    exit 1
fi
if ! ldd "$bin" | grep -q 'libgstreamer-1\.0\.so'; then
    echo "$bin was built without --features video" >&2
    exit 1
fi

version=$(cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["packages"][0]["version"])')
dist=dist/linux
rm -rf "$dist"
mkdir -p "$dist"

# --- .deb -------------------------------------------------------------------
cargo deb --no-build --output "$dist/cerno_${version}_amd64.deb"

# --- AppImage ---------------------------------------------------------------
# Pinned tools, checked by hash. linuxdeploy collects the libraries, appimagetool
# packs the AppDir with the static type-2 runtime (no libfuse2 on the user's side).
tools=target/appimage-tools
mkdir -p "$tools"
fetch() { # name url sha256
    local file="$tools/$1"
    if [[ ! -f $file ]] || ! echo "$3  $file" | sha256sum --check --status; then
        curl -fsSL --retry 3 -o "$file" "$2"
        echo "$3  $file" | sha256sum --check --quiet
    fi
    chmod +x "$file"
}
fetch linuxdeploy \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
    c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
fetch appimagetool \
    https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage \
    ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
fetch runtime \
    https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64 \
    2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

appdir=target/appimage/Cerno.AppDir
rm -rf "$appdir"
plugins=$appdir/usr/lib/libheif/plugins
doc=$appdir/usr/share/doc/cerno
mkdir -p "$appdir/usr/bin" "$plugins" "$doc" "$appdir/usr/share/metainfo"
install -m755 -s "$bin" "$appdir/usr/bin/cerno"
install -m644 packaging/linux/io.github.fly2nbc_oss.cerno.metainfo.xml "$appdir/usr/share/metainfo/"
# linuxdeploy also deploys the plugin's own dependency, libde265, into usr/lib.
install -m644 "$(dpkg -L libheif-plugin-libde265 | grep '/libheif-libde265\.so$')" "$plugins/"
cp LICENSE NOTICE THIRD_PARTY.md "$doc/"
cp -r target/release/licenses "$doc/licenses"

# The tools are AppImages themselves; CI runners have no FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1
# Ubuntu's libraries are stripped already, and linuxdeploy's own strip is older
# than the toolchain (it fails on newer ELF sections).
export NO_STRIP=1
# GStreamer and GLib come from the host, the whole family: a bundled libgstreamer next
# to the host's plugins (built against another GStreamer and GLib) would fail to load
# them. Ubuntu's libavcodec is built with GPL parts, so it must not travel with Cerno
# either.
"$tools/linuxdeploy" --appdir "$appdir" \
    --executable "$appdir/usr/bin/cerno" \
    --desktop-file packaging/linux/cerno.desktop \
    --icon-file assets/icon.png --icon-filename cerno \
    --custom-apprun packaging/linux/AppRun \
    --exclude-library 'libgst*' \
    --exclude-library 'libglib-2.0.so*' --exclude-library 'libgobject-2.0.so*' \
    --exclude-library 'libgmodule-2.0.so*' --exclude-library 'libgio-2.0.so*' \
    --exclude-library 'liborc-0.4.so*'
if find "$appdir/usr/lib" -name '*.so*' -printf '%f\n' |
    grep -E '^lib(gst|glib-2|gobject-2|gmodule-2|gio-2|orc-0|av(codec|format|util))'; then
    echo "the AppImage bundles GStreamer, GLib or FFmpeg - they must come from the host" >&2
    exit 1
fi
# linuxdeploy gives every library `$ORIGIN`, but the plugin sits two levels below
# usr/lib, where libheif and libde265 are.
# shellcheck disable=SC2016 # $ORIGIN is for the dynamic loader, not for the shell
patchelf --set-rpath '$ORIGIN/../..' "$plugins/libheif-libde265.so"

# Both must resolve to the AppDir, or the AppImage would silently use the host's
# libheif (or none): HEIC photos would then fail to open. ldd prints the rpath
# unresolved (usr/bin/../lib/...), so compare canonical paths.
root=$(readlink -f "$appdir")
bundled() { # elf soname-regex
    local path
    path=$(ldd "$1" | awk -v lib="$2" '$1 ~ lib { print $3; exit }')
    [[ -n $path && $(readlink -f "$path") == "$root"/usr/lib/* ]]
}
bundled "$appdir/usr/bin/cerno" '^libheif[.]so' ||
    { echo "cerno does not load the bundled libheif" >&2; exit 1; }
bundled "$plugins/libheif-libde265.so" '^libde265[.]so' ||
    { echo "the HEVC plugin does not load the bundled libde265" >&2; exit 1; }

# The LGPL libraries' versions, so their source can be found (Ubuntu source packages).
# shellcheck disable=SC2016 # ${Package} and ${Version} are dpkg-query's format fields
find "$appdir/usr/lib" -name '*.so*' -type f -printf '%f\n' | sort |
    while read -r lib; do { dpkg -S "*/$lib" 2>/dev/null || true; } | cut -d: -f1; done |
    sort -u | xargs dpkg-query -W -f='${Package} ${Version}\n' >"$doc/bundled-libraries.txt"
cat "$doc/bundled-libraries.txt"

# The update information points AppImageUpdate at the latest GitHub release's .zsync.
# appimagetool has zsyncmake write it into the working directory, under the AppImage's
# name, so it runs in dist/linux. Cerno itself has no updater.
# --no-appstream: appimagetool looks only for a metainfo named after the desktop file
# (cerno.appdata.xml); CI's lint job checks ours with appstreamcli.
appimage=$dist/cerno_${version}_x86_64.AppImage
here=$PWD
(cd "$dist" && ARCH=x86_64 "$here/$tools/appimagetool" --no-appstream \
    --runtime-file "$here/$tools/runtime" \
    --updateinformation "gh-releases-zsync|fly2nbc-oss|Cerno|latest|cerno_*_x86_64.AppImage.zsync" \
    "$here/$appdir" "${appimage##*/}")
if [[ ! -s $appimage.zsync ]]; then
    echo "appimagetool wrote no $appimage.zsync - is zsyncmake installed?" >&2
    exit 1
fi

# The AppImage starts and knows its version (no window, no data folder).
said=$("$appimage" --version)
if [[ $said != "cerno $version" ]]; then
    echo "the AppImage says '$said' to --version, expected 'cerno $version'" >&2
    exit 1
fi

(cd "$dist" && ls -l && sha256sum -- *)
