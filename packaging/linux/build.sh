#!/usr/bin/env bash
# Packs a Linux release build into dist/linux:
#
#   cerno_<version>_amd64.deb        cargo-deb; depends on the system's libheif
#   cerno_<version>_x86_64.AppImage  portable; bundles libheif and its libde265 plugin
#
# Run after `cargo build --release --features heic`, on Ubuntu 24.04: it is the
# oldest base with libheif >= 1.17, and the AppImage runs on distributions with
# its glibc (2.39) or newer.
# Needs: cargo-deb, patchelf, libheif-plugin-libde265, curl.
set -euo pipefail
cd "$(dirname "$0")/../.."

bin=target/release/cerno
if [[ ! -x $bin ]]; then
    echo "$bin is missing - run: cargo build --release --features heic" >&2
    exit 1
fi
if ! ldd "$bin" | grep -q 'libheif\.so'; then
    echo "$bin was built without --features heic" >&2
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
mkdir -p "$appdir/usr/bin" "$plugins" "$doc"
install -m755 -s "$bin" "$appdir/usr/bin/cerno"
# linuxdeploy also deploys the plugin's own dependency, libde265, into usr/lib.
install -m644 "$(dpkg -L libheif-plugin-libde265 | grep '/libheif-libde265\.so$')" "$plugins/"
cp LICENSE NOTICE THIRD_PARTY.md "$doc/"
cp -r target/release/licenses "$doc/licenses"

# The tools are AppImages themselves; CI runners have no FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1
# Ubuntu's libraries are stripped already, and linuxdeploy's own strip is older
# than the toolchain (it fails on newer ELF sections).
export NO_STRIP=1
"$tools/linuxdeploy" --appdir "$appdir" \
    --executable "$appdir/usr/bin/cerno" \
    --desktop-file packaging/linux/cerno.desktop \
    --icon-file assets/icon.png --icon-filename cerno \
    --custom-apprun packaging/linux/AppRun
# linuxdeploy gives every library `$ORIGIN`, but the plugin sits two levels below
# usr/lib, where libheif and libde265 are.
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
find "$appdir/usr/lib" -name '*.so*' -type f -printf '%f\n' | sort |
    while read -r lib; do { dpkg -S "*/$lib" 2>/dev/null || true; } | cut -d: -f1; done |
    sort -u | xargs dpkg-query -W -f='${Package} ${Version}\n' >"$doc/bundled-libraries.txt"
cat "$doc/bundled-libraries.txt"

ARCH=x86_64 "$tools/appimagetool" --no-appstream --runtime-file "$tools/runtime" \
    "$appdir" "$dist/cerno_${version}_x86_64.AppImage"

(cd "$dist" && ls -l && sha256sum -- *)
