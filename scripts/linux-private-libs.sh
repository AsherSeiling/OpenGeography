#!/usr/bin/env bash
# Linux AppImage: keep conda-forge's GDAL stack private.
# The AppImage bundler flattens every library into one usr/lib, so conda's and
# Ubuntu's same-named libraries (libjpeg, libpcre2, libstdc++, ...) overwrite each
# other and one side crashes. This rewrites the image so that
#   * GDAL's whole dependency closure lives in usr/lib/gdal-private, each library
#     pointing at its own folder (DT_RPATH, which beats LD_LIBRARY_PATH), and
#   * usr/lib keeps the system versions for WebKitGTK and the rest.
#   scripts/linux-private-libs.sh <app.AppImage> <conda prefix>   (rewrites in place)
set -euo pipefail
APPIMAGE="$(readlink -f "$1")"; CONDA="$(readlink -f "$2")"
WORK="$(mktemp -d)"
cd "$WORK"
chmod +x "$APPIMAGE"
OFFSET=$("$APPIMAGE" --appimage-offset)
head -c "$OFFSET" "$APPIMAGE" > runtime
"$APPIMAGE" --appimage-extract >/dev/null
ROOT="$WORK/squashfs-root"
PRIV="$ROOT/usr/lib/gdal-private"
mkdir -p "$PRIV"

# Closure of the conda libraries the app links (ldd resolves them through conda's own RUNPATHs).
BIN=$(find "$ROOT/usr/bin" -maxdepth 1 -type f -perm -u+x | head -1)
ROOTS=$(ls "$CONDA"/lib/libgdal.so.* "$CONDA"/lib/libproj.so.* "$CONDA"/lib/libgeos_c.so.* 2>/dev/null | head -20)
[ -n "$ROOTS" ] || { echo "no conda GDAL/PROJ/GEOS libraries under $CONDA/lib" >&2; exit 1; }
# Glibc and the loader always come from the system.
SKIP='^(libc|libm|libdl|libpthread|librt|libresolv|libutil|libnsl|ld-linux.*)\.so'
for f in $ROOTS; do
  ldd "$f" | awk '/=> \//{print $3}'
done | sort -u | while read -r lib; do
  case "$(readlink -f "$lib")" in "$CONDA"/*) ;; *) continue ;; esac
  name=$(basename "$lib")
  echo "$name" | grep -Eq "$SKIP" && continue
  cp -L "$lib" "$PRIV/$name"
done
for f in $ROOTS; do cp -L "$f" "$PRIV/$(basename "$f")"; done
chmod u+w "$PRIV"/*

# usr/lib must hold the system versions: put them back where the flat copy came from conda.
for f in "$PRIV"/*; do
  name=$(basename "$f"); flat="$ROOT/usr/lib/$name"
  [ -e "$flat" ] || continue
  sys=""
  for d in /lib/x86_64-linux-gnu /usr/lib/x86_64-linux-gnu; do [ -e "$d/$name" ] && sys="$d/$name" && break; done
  if [ -n "$sys" ]; then cp -L "$sys" "$flat"; elif cmp -s "$flat" "$CONDA/lib/$name"; then :; fi
done

# The dynamic loader reuses an already-loaded library with the same soname, so
# rpaths alone don't isolate: WebKitGTK's libjpeg.so.8 (loaded first) would be
# handed to conda's libtiff. Give every private library a unique soname.
ORIG=$(ls "$PRIV")
for name in $ORIG; do mv "$PRIV/$name" "$PRIV/gs_$name"; done
rename_needed() {  # <elf file>
  for dep in $(patchelf --print-needed "$1"); do
    echo "$ORIG" | grep -qx "$dep" && patchelf --replace-needed "$dep" "gs_$dep" "$1"
  done
  return 0
}
for f in "$PRIV"/*; do
  patchelf --set-soname "$(basename "$f")" "$f"
  rename_needed "$f"
  patchelf --force-rpath --set-rpath '$ORIGIN' "$f"
done
rename_needed "$BIN"

# The app finds GDAL in the private folder (and everything else in usr/lib).
patchelf --force-rpath --set-rpath '$ORIGIN/../lib/gdal-private:$ORIGIN/../lib' "$BIN"
echo "Private libraries: $(ls "$PRIV" | wc -l) files, $(du -sh "$PRIV" | cut -f1)"
echo "ldd of the app's GDAL resolution:"; LD_LIBRARY_PATH="$ROOT/usr/lib" ldd "$BIN" | grep -E 'gdal|proj|geos|jpeg|tiff|stdc' || true

# Repack with the original runtime.
curl -fsSL -o appimagetool "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod +x appimagetool
ARCH=x86_64 ./appimagetool --appimage-extract-and-run --runtime-file runtime "$ROOT" new.AppImage >/dev/null
chmod +x new.AppImage
mv new.AppImage "$APPIMAGE"
echo "Rewrote $APPIMAGE"
