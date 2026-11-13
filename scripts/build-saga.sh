#!/usr/bin/env bash
# Build SAGA GIS's command-line program `saga_cmd` (GPL-2.0-or-later; tool libraries GPL-2+ / GPL-3+, saga_api LGPL-2.1+) from a pinned release
# WITHOUT the GUI and WITHOUT any change to its source, and stage it as a separate program for the terrain / grid tools:
#   src-tauri/resources/saga/{saga_cmd, lib/, tools/, licenses/, SOURCE_OFFER.txt}
# GIS Studio only starts it and exchanges files with it (like WhiteboxTools, `whitebox.rs`): aggregation, never linking. DECISIONS "GPL tools as separate programs".
# Source: https://github.com/saga-gis/saga-gis at tag saga-9.5.1 (commit below).
# Dependencies: wxWidgets' *base* library only (wxWindows Licence = LGPL-2 + exception), built here as static libraries from the pinned 3.3.4 release,
# no GTK/Cocoa GUI. Tool libraries that need GDAL, PDAL, ODBC, curl or the wx GUI are not staged: files go in and out as SAGA's own .sgrd/.sdat grids
# and shapefiles, which GIS Studio writes and reads itself, so nothing outside the system (and libomp on macOS) is needed at run time.
#   scripts/build-saga.sh                 fetch the pinned sources into $SAGA_WORK, build, stage
#   SAGA_SRC=<checkout>                   use an existing checkout (offline); its commit must match
#   WX_PREFIX=<dir>                       use an existing static wxBase install (skips the wx build); on Windows use `vcpkg install wxwidgets` and this
#   SAGA_WORK=<dir>                       build folder (default target/saga-build); SAGA_REBUILD=1 forces a rebuild
# Verified: macOS arm64 (2026-11-12). Linux and Windows steps below are written, NOT yet run (see docs/SAGA_PLAN.md).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAG="saga-9.5.1"
COMMIT="00530d3630a5aa4d30d5e1219587db979daf8588"
WX_VER="3.3.4"
WX_SHA256="815c16cafa4fcbfcf25c5c0a3418309c62257b8a3b6f91d85a1367a134f98c98"
OUT="$ROOT/src-tauri/resources/saga"
WORK="${SAGA_WORK:-$ROOT/target/saga-build}"
OS="$(uname -s)"; case "$OS" in MINGW*|MSYS*|CYGWIN*) OS=Windows ;; esac
EXE="saga_cmd"; [ "$OS" = Windows ] && EXE="saga_cmd.exe"
case "$OS" in Darwin) DLL="dylib" ;; Windows) DLL="dll" ;; *) DLL="so" ;; esac
JOBS="$( (sysctl -n hw.ncpu || nproc || echo 4) 2>/dev/null )"

# In CI the tests find the program through GIS_STUDIO_SAGA (a native path on Windows).
export_env() { if [ -n "${GITHUB_ENV:-}" ]; then P="$OUT/$EXE"; command -v cygpath >/dev/null && P="$(cygpath -w "$P")"; echo "GIS_STUDIO_SAGA=$P" >> "$GITHUB_ENV"; fi; }
if [ -x "$OUT/$EXE" ] && [ -z "${SAGA_REBUILD:-}" ]; then echo "SAGA already staged ($OUT/$EXE)"; export_env; exit 0; fi
mkdir -p "$WORK"

# 1. SAGA source at the pinned commit
SRC="${SAGA_SRC:-$WORK/saga-gis}"
HAVE="$(git -C "$SRC" rev-parse HEAD 2>/dev/null || true)"
if [ "$HAVE" != "$COMMIT" ] && [ -z "${SAGA_SRC:-}" ]; then
  rm -rf "$SRC"
  git clone -q --depth 1 --branch "$TAG" https://github.com/saga-gis/saga-gis.git "$SRC"
  HAVE="$(git -C "$SRC" rev-parse HEAD)"
fi
[ "$HAVE" = "$COMMIT" ] || { echo "SAGA checkout is $HAVE, expected $COMMIT ($TAG)"; exit 1; }

# 2. wxWidgets base (no GUI), static, position independent. SAGA's CMake asks for the components base, net, xml AND html; html lives in the GUI
#    library and SAGA's sources never include it, so a build without a GUI gets an empty stand-in archive (build folder only, SAGA is untouched).
if [ -z "${WX_PREFIX:-}" ]; then
  WX_PREFIX="$WORK/wx-install"
  if [ ! -x "$WX_PREFIX/bin/wx-config" ]; then
    curl -fsSL -o "$WORK/wx.tar.bz2" "https://github.com/wxWidgets/wxWidgets/releases/download/v$WX_VER/wxWidgets-$WX_VER.tar.bz2"
    echo "$WX_SHA256  $WORK/wx.tar.bz2" | shasum -a 256 -c -
    rm -rf "$WORK/wx-src" "$WORK/wx-build"; mkdir -p "$WORK/wx-src" "$WORK/wx-build"
    tar xjf "$WORK/wx.tar.bz2" -C "$WORK/wx-src" --strip-components=1
    EXTRA=""; [ "$OS" = Darwin ] && EXTRA="--with-macosx-version-min=11.0"
    ( cd "$WORK/wx-build" && ../wx-src/configure --prefix="$WX_PREFIX" --disable-gui --disable-shared --disable-debug \
        --with-expat=builtin --with-zlib=builtin --with-regex=builtin --without-libcurl --without-libiconv $EXTRA CFLAGS=-fPIC CXXFLAGS=-fPIC >/dev/null && make -j"$JOBS" >/dev/null && make install >/dev/null )
    mkdir -p "$WORK/wx-license"; cp "$WORK/wx-src/docs/licence.txt" "$WORK/wx-src/docs/lgpl.txt" "$WORK/wx-src/docs/gpl.txt" "$WORK/wx-license/"
  fi
  HTML_LIB="$(ls "$WX_PREFIX"/lib/libwx_baseu-*.a | head -1 | sed 's/libwx_baseu-/libwx_baseu_html-/')"
  if [ ! -f "$HTML_LIB" ]; then
    echo 'void gis_studio_wxhtml_stand_in(void){}' > "$WORK/standin.c"; cc -c "$WORK/standin.c" -o "$WORK/standin.o"; ar rcs "$HTML_LIB" "$WORK/standin.o"
  fi
  WXCFG="-DwxWidgets_CONFIG_EXECUTABLE=$WX_PREFIX/bin/wx-config"
else
  WXCFG="-DwxWidgets_ROOT_DIR=$WX_PREFIX"   # Windows / vcpkg: cmake finds it with the toolchain file the caller passes in CMAKE_ARGS
fi

# 3. SAGA: saga_cmd and the tool libraries, no GUI. `make -k`: tools that need the wx GUI fail and are simply not staged.
BUILD="$WORK/saga-build"; rm -rf "$BUILD"; mkdir -p "$BUILD"
OMP=""; [ "$OS" = Darwin ] && OMP="-DOpenMP_ROOT=${LIBOMP_PREFIX:-/opt/homebrew/opt/libomp} -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0"
# shellcheck disable=SC2086
cmake -S "$SRC/saga-gis" -B "$BUILD" -DCMAKE_BUILD_TYPE=Release -DWITH_GUI=OFF -DWITH_PYTHON=OFF -DWITH_TOOLS_GDAL=OFF -DWITH_TOOLS_PDAL=OFF -DWITH_TOOLS_PROJ=OFF -DWITH_TOOLS_OPENCV=OFF -DWITH_TOOLS_VIGRA=OFF -DWITH_TOOLS_HPDF=OFF -DWITH_TOOLS_POSTGRES=OFF -DWITH_FIRE_SPREADING=OFF -DCMAKE_IGNORE_PREFIX_PATH=/opt/local $WXCFG $OMP ${CMAKE_ARGS:-} >/dev/null
cmake --build "$BUILD" -j "$JOBS" -- -k >/dev/null 2>&1 || true
CMD="$(find "$BUILD" -name "$EXE" -type f | head -1)"; [ -n "$CMD" ] || { echo "saga_cmd did not build"; exit 1; }

# 4. Stage. Tool libraries from this list only (no GDAL/PDAL/ODBC/curl/GUI ones); each must link nothing outside the system and the staged libs.
KEEP="ta_* grid_* grids_tools shapes_* tin_tools table_* statistics_grid statistics_points statistics_regression pointcloud_tools pj_geotrans io_grid io_shapes io_table imagery_tools climate_tools"
rm -rf "$OUT"; mkdir -p "$OUT/lib" "$OUT/tools" "$OUT/licenses"
cp "$CMD" "$OUT/$EXE"
cp "$(find "$BUILD" -name "libsaga_api.*$DLL*" -o -name "libsaga_api*.$DLL" | head -1)" "$OUT/lib/" 2>/dev/null || true
find "$BUILD" -name "libsaga_api*" \( -type f -o -type l \) -exec cp -P {} "$OUT/lib/" \;
for pat in $KEEP; do find "$BUILD/src/tools" -name "lib${pat}.$DLL" -type f -exec cp {} "$OUT/tools/" \; ; done
if [ "$OS" = Darwin ]; then
  OMPLIB="${LIBOMP_PREFIX:-/opt/homebrew/opt/libomp}/lib/libomp.dylib"; cp -L "$OMPLIB" "$OUT/lib/libomp.dylib"; chmod u+w "$OUT/lib/libomp.dylib"
  cp "${LIBOMP_PREFIX:-/opt/homebrew/opt/libomp}/LICENSE.TXT" "$OUT/licenses/LLVM-OpenMP-LICENSE.txt"
  strip_rpaths() { for r in $(otool -l "$1" | awk '/LC_RPATH/{f=1} f&&/path /{print $2; f=0}'); do install_name_tool -delete_rpath "$r" "$1"; done; }
  fix() { # $1 file, $2 rpath
    strip_rpaths "$1"; install_name_tool -add_rpath "$2" "$1" 2>/dev/null
    install_name_tool -change "$OMPLIB" "@rpath/libomp.dylib" "$1" 2>/dev/null || true
    for o in $(otool -L "$1" | awk '/libomp.dylib/{print $1}'); do install_name_tool -change "$o" "@rpath/libomp.dylib" "$1" 2>/dev/null || true; done
  }
  fix "$OUT/$EXE" "@executable_path/lib"
  for f in "$OUT"/lib/libsaga_api*.dylib; do [ -L "$f" ] || fix "$f" "@loader_path"; done
  install_name_tool -id "@rpath/libomp.dylib" "$OUT/lib/libomp.dylib"
  for f in "$OUT"/tools/*.dylib; do fix "$f" "@loader_path/../lib"; done
  codesign --force --sign - "$OUT/$EXE" "$OUT"/lib/*.dylib "$OUT"/tools/*.dylib 2>/dev/null || true   # ad hoc; the app bundle signing re-signs
  BAD="$( (otool -L "$OUT/$EXE" "$OUT"/lib/*.dylib "$OUT"/tools/*.dylib | awk '/^\t/{print $1}' | grep -v -E '^(@rpath/|/usr/lib/|/System/)' | sort -u) || true )"
elif [ "$OS" = Linux ]; then
  command -v patchelf >/dev/null || { echo "patchelf is needed to set \$ORIGIN rpaths"; exit 1; }
  patchelf --set-rpath '$ORIGIN/lib' "$OUT/$EXE"
  for f in "$OUT"/lib/libsaga_api*.so*; do [ -L "$f" ] || patchelf --set-rpath '$ORIGIN' "$f"; done
  for f in "$OUT"/tools/*.so; do patchelf --set-rpath '$ORIGIN/../lib' "$f"; done
  # libgomp (GCC runtime library: GPL-3 with the Runtime Library Exception) is copied next to saga_api so no gcc install is needed.
  cp -L "$(g++ -print-file-name=libgomp.so.1)" "$OUT/lib/"
  BAD="$( (ldd "$OUT/$EXE" "$OUT"/lib/*.so* "$OUT"/tools/*.so | awk '/=>/{print $3}' | grep -v -E '/(libc|libm|libdl|libpthread|librt|libgcc_s|libstdc\+\+|ld-linux[^/]*)\.so' | grep -v -E "$OUT|not" | sort -u) || true )"
else  # Windows (MSYS/MSVC): saga_api.dll next to saga_cmd.exe, tools in tools/; wx is linked statically by vcpkg's x64-windows-static triplet
  cp "$(find "$BUILD" -name "saga_api.dll" | head -1)" "$OUT/"; BAD=""
fi
if [ -n "${BAD:-}" ]; then echo "A staged SAGA file links a library outside the system:"; echo "$BAD"; exit 1; fi

# 5. Licence texts, notices and the written source offer (GPL-2/3 section 3(b)/6 alternatives: we also ship the pinned commit and this script).
cp "$SRC/saga-gis/src/gpl.txt" "$OUT/licenses/SAGA-GPL-2.txt"; cp "$SRC/saga-gis/src/lgpl.txt" "$OUT/licenses/SAGA-API-LGPL-2.1.txt"
[ -d "$WORK/wx-license" ] && { cp "$WORK/wx-license/licence.txt" "$OUT/licenses/wxWidgets-licence.txt"; cp "$WORK/wx-license/gpl.txt" "$OUT/licenses/GPL-3-reference.txt"; }
cat > "$OUT/SOURCE_OFFER.txt" <<EOF
SAGA GIS $TAG (https://saga-gis.sourceforge.io, https://github.com/saga-gis/saga-gis) is free software: GPL version 2 or later for saga_cmd and most tool
libraries (some tool libraries are GPL version 3 or later; the combined program is distributed under GPL version 3 or later), LGPL 2.1 or later for
libsaga_api. It is included here UNMODIFIED, as a separate program that GIS Studio starts and exchanges files with. GIS Studio's own code is not
part of it and is not covered by the GPL.

Corresponding source: https://github.com/saga-gis/saga-gis, tag $TAG, commit $COMMIT. Build instructions: scripts/build-saga.sh in the GIS Studio
source (https://github.com/ - the project repository). wxWidgets $WX_VER (wxWindows Licence: LGPL 2 with an exception for binaries), statically linked:
https://github.com/wxWidgets/wxWidgets/releases/tag/v$WX_VER. On request, we will also send the source on a physical medium for the cost of copying.
EOF
export_env
echo "SAGA $TAG staged in $OUT ($(du -sh "$OUT" | cut -f1), $(ls "$OUT/tools" | wc -l | tr -d ' ') tool libraries)"
