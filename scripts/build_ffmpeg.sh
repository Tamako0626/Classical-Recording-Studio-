#!/usr/bin/env bash
# 给 GitHub Actions 编裁剪版 ffmpeg 与 ffprobe (GPL, 因 libx264). 不改上游源码.
# 不要在开发机上跑. 成品由 ffmpeg.yml 挂到 sidecars release.
# Windows 用 cl / --toolchain=msvc, 成品不带 msys dll. MSYS2 只提供 GNU make 和 pkg-config.
set -euo pipefail

FFMPEG_TAG="${FFMPEG_TAG:-n7.1.1}"
X264_REF="${X264_REF:-stable}"
LAME_TAG="${LAME_TAG:-3.100}"
ZLIB_TAG="${ZLIB_TAG:-1.3.1}"
FFMPEG_URL="https://github.com/FFmpeg/FFmpeg/archive/refs/tags/${FFMPEG_TAG}.tar.gz"
X264_URL="https://github.com/mirror/x264/archive/refs/heads/${X264_REF}.tar.gz"
LAME_URL="https://downloads.sourceforge.net/project/lame/lame/${LAME_TAG}/lame-${LAME_TAG}.tar.gz"
ZLIB_URL="https://github.com/madler/zlib/releases/download/v${ZLIB_TAG}/zlib-${ZLIB_TAG}.tar.gz"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$ROOT/vendor"
WORK_DIR=""
ARCH=""
JOBS=""
DO_UPX=""
TARGET="native"

usage() {
  echo "usage: $0 [--out-dir DIR] [--work-dir DIR] [--arch arm64|x86_64] [--target native|msvc] [--jobs N] [--skip-upx]" >&2
  exit 2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --out-dir) OUT_DIR="$2"; shift 2 ;;
    --work-dir) WORK_DIR="$2"; shift 2 ;;
    --arch) ARCH="$2"; shift 2 ;;
    --target) TARGET="$2"; shift 2 ;;
    --jobs) JOBS="$2"; shift 2 ;;
    --skip-upx) DO_UPX=0; shift ;;
    -h|--help) usage ;;
    *) echo "unknown arg: $1" >&2; usage ;;
  esac
done

host_arch() {
  case "$(uname -m)" in
    arm64|aarch64) echo arm64 ;;
    x86_64|amd64) echo x86_64 ;;
    *) uname -m ;;
  esac
}

is_windows() {
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) return 0 ;;
    *) return 1 ;;
  esac
}

is_macos() {
  [ "$(uname -s)" = Darwin ]
}

if [ -z "$WORK_DIR" ]; then
  if [ -n "${RUNNER_TEMP:-}" ]; then
    WORK_DIR="$RUNNER_TEMP/crs-ffmpeg"
  else
    WORK_DIR="${HOME}/.cache/crs-ffmpeg"
  fi
fi

if ! command -v make >/dev/null 2>&1; then
  echo "need GNU make" >&2
  exit 1
fi

if [ -z "$ARCH" ]; then
  ARCH="$(host_arch)"
fi
if [ -z "$JOBS" ]; then
  n="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 8)"
  if [ "$n" -gt 12 ]; then
    JOBS=12
  else
    JOBS="$n"
  fi
fi

MSVC=0
if [ "$TARGET" = msvc ]; then
  MSVC=1
  ARCH=x86_64
  if ! command -v cl >/dev/null 2>&1 && ! command -v cl.exe >/dev/null 2>&1; then
    echo "need cl.exe (ilammy/msvc-dev-cmd)" >&2
    exit 1
  fi
elif [ "$TARGET" != native ]; then
  echo "unknown --target $TARGET (native|msvc)" >&2
  exit 2
fi

if [ "$MSVC" = 1 ]; then
  for req in tar curl nasm; do
    if ! command -v "$req" >/dev/null 2>&1; then
      echo "need $req on PATH" >&2
      exit 1
    fi
  done
else
  for req in gcc tar curl nasm; do
    if ! command -v "$req" >/dev/null 2>&1; then
      if [ "$req" = gcc ] && is_macos && command -v clang >/dev/null 2>&1; then
        continue
      fi
      echo "need $req on PATH" >&2
      exit 1
    fi
  done
fi

TARGET_WIN=0
EXE_SUFFIX=""
if is_windows || [ "$MSVC" = 1 ]; then
  TARGET_WIN=1
  EXE_SUFFIX=".exe"
fi
if [ -z "$DO_UPX" ]; then
  if [ "$TARGET_WIN" = 1 ] || [ "$(uname -s)" = Linux ]; then
    DO_UPX=1
  else
    DO_UPX=0
  fi
fi

CROSS=0
if is_macos && [ "$ARCH" != "$(host_arch)" ]; then
  CROSS=1
fi

mkdir -p "$OUT_DIR" "$WORK_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
WORK_DIR="$(cd "$WORK_DIR" && pwd)"
PREFIX="$WORK_DIR/prefix"
SRC="$WORK_DIR/src"
mkdir -p "$PREFIX" "$SRC"
OUT_BIN="$OUT_DIR/ffmpeg${EXE_SUFFIX}"
OUT_PROBE="$OUT_DIR/ffprobe${EXE_SUFFIX}"

echo "==> arch=$ARCH jobs=$JOBS target=$TARGET cross=$CROSS upx=$DO_UPX"
echo "==> work=$WORK_DIR"

fetch_tar() {
  local url="$1" dest="$2" marker="$3"
  if [ -f "$marker" ]; then
    echo "==> reuse $dest"
    return 0
  fi
  local tmp="$dest.tar.download"
  echo "==> download $url"
  curl -fL --retry 5 --retry-delay 2 -o "$tmp" "$url"
  mkdir -p "$dest"
  tar -xf "$tmp" -C "$dest" --strip-components=1
  rm -f "$tmp"
  : >"$marker"
}

wipe_build_tree() {
  local dir="$1"
  [ -d "$dir" ] || return 0
  if [ -f "$dir/Makefile" ] || [ -f "$dir/ffbuild/config.mak" ] || [ -f "$dir/config.mak" ]; then
    make -C "$dir" distclean >/dev/null 2>&1 || make -C "$dir" clean >/dev/null 2>&1 || true
  fi
  find "$dir" -type f \( \
    -name '*.o' -o -name '*.obj' -o -name '*.a' -o -name '*.lib' \
    -o -name '*.exe' -o -name 'ffmpeg' -o -name 'ffprobe' -o -name 'x264' \
  \) -delete 2>/dev/null || true
}

rewrite_msvc_pc() {
  local pc="$1" fallback_lib="$2"
  [ "$MSVC" = 1 ] || return 0
  if [ ! -f "$pc" ]; then
    echo "pkg-config file missing: $pc" >&2
    exit 1
  fi
  local prefix_mix lib="$fallback_lib" tmp
  prefix_mix="$(cygpath -m "$PREFIX")"
  if [ -f "$PREFIX/lib/${fallback_lib}-static.lib" ]; then
    lib="${fallback_lib}-static"
  fi
  tmp="$pc.tmp"
  awk -v p="$prefix_mix" -v lib="$lib" '
    /^prefix=/ { print "prefix=" p; next }
    /^exec_prefix=/ { print "exec_prefix=" p; next }
    /^libdir=/ { print "libdir=" p "/lib"; next }
    /^includedir=/ { print "includedir=" p "/include"; next }
    /^Libs:/ { print "Libs: -L" p "/lib -l" lib; next }
    /^Libs.private:/ { next }
    /^Requires.private:/ { next }
    { print }
  ' "$pc" >"$tmp"
  mv "$tmp" "$pc"
}

cmake_static() {
  local src_arg="$1" build_arg="$2"
  shift 2
  local prefix_arg="$PREFIX"
  local -a gen=(-G "Unix Makefiles")
  local -a msvc_cc=()
  if [ "$MSVC" = 1 ]; then
    export MSYS2_ARG_CONV_EXCL='*'
    src_arg="$(cygpath -w "$src_arg")"
    build_arg="$(cygpath -w "$build_arg")"
    prefix_arg="$(cygpath -w "$PREFIX")"
    local cc
    cc="$(command -v cl.exe || command -v cl)"
    cc="$(cygpath -w "$cc")"
    msvc_cc=(
      -DCMAKE_C_COMPILER="$cc"
      -DCMAKE_CXX_COMPILER="$cc"
      -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded
      "-DCMAKE_C_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG"
      "-DCMAKE_CXX_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG"
    )
    if command -v ninja >/dev/null 2>&1 || command -v ninja.exe >/dev/null 2>&1; then
      gen=(-G Ninja)
    else
      gen=(-G "NMake Makefiles")
    fi
  fi
  cmake -S "$src_arg" -B "$build_arg" "${gen[@]}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$prefix_arg" \
    -DCMAKE_INSTALL_LIBDIR=lib \
    "${msvc_cc[@]}" \
    "$@"
  if [ "$MSVC" = 1 ]; then
    cmake --build "$build_arg"
    cmake --install "$build_arg"
    unset MSYS2_ARG_CONV_EXCL
  else
    cmake --build "$build_arg" -j"$JOBS"
    cmake --install "$build_arg"
  fi
}

install_zlib_msvc() {
  fetch_tar "$ZLIB_URL" "$SRC/zlib" "$SRC/zlib.ok"
  if [ -f "$PREFIX/lib/zlibstatic.lib" ]; then
    echo "==> reuse zlib"
  else
    echo "==> build zlib $ZLIB_TAG"
    rm -rf "$SRC/zlib/build"
    cmake_static "$SRC/zlib" "$SRC/zlib/build" -DBUILD_SHARED_LIBS=OFF
  fi
  if [ ! -f "$PREFIX/lib/zlibstatic.lib" ]; then
    echo "zlib static library missing" >&2
    exit 1
  fi
  # CMake also installs an import library named zlib.lib for zlib.dll.
  rm -f "$PREFIX/lib/zlib.lib" "$PREFIX/lib/z.lib" "$PREFIX/bin/zlib.dll" "$PREFIX/bin/zlib1.dll"
  cp -f "$PREFIX/lib/zlibstatic.lib" "$PREFIX/lib/zlib.lib"
  cp -f "$PREFIX/lib/zlibstatic.lib" "$PREFIX/lib/z.lib"
  if [ ! -f "$PREFIX/lib/pkgconfig/zlib.pc" ]; then
    mkdir -p "$PREFIX/lib/pkgconfig"
    cat >"$PREFIX/lib/pkgconfig/zlib.pc" <<EOF
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: zlib
Description: zlib
Version: ${ZLIB_TAG}
Libs: -L\${libdir} -lz
Cflags: -I\${includedir}
EOF
  fi
  rewrite_msvc_pc "$PREFIX/lib/pkgconfig/zlib.pc" z
}

install_lame() {
  fetch_tar "$LAME_URL" "$SRC/lame" "$SRC/lame.ok"
  if [ -f "$PREFIX/lib/libmp3lame.a" ] || [ -f "$PREFIX/lib/mp3lame.lib" ] || [ -f "$PREFIX/lib/libmp3lame.lib" ]; then
    echo "==> reuse lame"
    rewrite_msvc_pc "$PREFIX/lib/pkgconfig/libmp3lame.pc" mp3lame
    return 0
  fi
  echo "==> build lame $LAME_TAG"
  if [ "$MSVC" = 1 ]; then
    if [ ! -f "$SRC/lame/configMS.h" ]; then
      echo "lame configMS.h missing" >&2
      exit 1
    fi
    cp -f "$SRC/lame/configMS.h" "$SRC/lame/config.h"
    local obj="$SRC/lame/msvc-obj"
    rm -rf "$obj"
    mkdir -p "$obj"
    local win src
    win="$(cygpath -w "$SRC/lame")"
    export MSYS2_ARG_CONV_EXCL='*'
    local sources=(
      bitstream.c encoder.c fft.c gain_analysis.c id3tag.c lame.c newmdct.c
      presets.c psymodel.c quantize.c quantize_pvt.c reservoir.c set_get.c
      tables.c takehiro.c util.c vbrquantize.c VbrTag.c version.c
    )
    local file
    for file in "${sources[@]}"; do
      src="$(cygpath -w "$SRC/lame/libmp3lame/$file")"
      ( cd "$obj" && cl /nologo /MT /O2 /DNDEBUG /DHAVE_CONFIG_H /D_CRT_SECURE_NO_WARNINGS \
        "/I${win}" "/I${win}\\include" "/I${win}\\libmp3lame" /c "$src" )
    done
    ( cd "$obj" && lib /nologo "/OUT:$(cygpath -w "$obj/mp3lame.lib")" *.obj )
    unset MSYS2_ARG_CONV_EXCL
    mkdir -p "$PREFIX/lib" "$PREFIX/include/lame" "$PREFIX/lib/pkgconfig"
    cp -f "$obj/mp3lame.lib" "$PREFIX/lib/mp3lame.lib"
    cp -f "$obj/mp3lame.lib" "$PREFIX/lib/libmp3lame.lib"
    cp -f "$SRC/lame/include/lame.h" "$PREFIX/include/lame/lame.h"
  else
    local saved_cc="${CC:-}" saved_cflags="${CFLAGS:-}" saved_ldflags="${LDFLAGS:-}"
    cd "$SRC/lame"
    if [ "$CROSS" = 1 ] && is_macos; then
      export CC="clang -arch $ARCH"
      export CFLAGS="-arch $ARCH"
      export LDFLAGS="-arch $ARCH"
      ./configure --prefix="$PREFIX" --host="${ARCH}-apple-darwin" --enable-static --disable-shared --disable-frontend
    else
      ./configure --prefix="$PREFIX" --enable-static --disable-shared --disable-frontend
    fi
    make -j"$JOBS"
    make install
    if [ -n "$saved_cc" ]; then export CC="$saved_cc"; else unset CC; fi
    if [ -n "$saved_cflags" ]; then export CFLAGS="$saved_cflags"; else unset CFLAGS; fi
    if [ -n "$saved_ldflags" ]; then export LDFLAGS="$saved_ldflags"; else unset LDFLAGS; fi
  fi
  if [ ! -f "$PREFIX/lib/pkgconfig/libmp3lame.pc" ]; then
    mkdir -p "$PREFIX/lib/pkgconfig"
    cat >"$PREFIX/lib/pkgconfig/libmp3lame.pc" <<EOF
prefix=$PREFIX
exec_prefix=\${prefix}
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: libmp3lame
Description: LAME
Version: ${LAME_TAG}
Libs: -L\${libdir} -lmp3lame
Cflags: -I\${includedir}
EOF
  fi
  rewrite_msvc_pc "$PREFIX/lib/pkgconfig/libmp3lame.pc" mp3lame
}

fetch_tar "$X264_URL" "$SRC/x264" "$SRC/x264.ok"
fetch_tar "$FFMPEG_URL" "$SRC/ffmpeg" "$SRC/ffmpeg.ok"
if [ "$MSVC" = 1 ]; then
  install_zlib_msvc
fi
install_lame
wipe_build_tree "$SRC/x264"
wipe_build_tree "$SRC/ffmpeg"

echo "==> build x264"
cd "$SRC/x264"
x264_cfg=(
  --prefix="$PREFIX"
  --enable-static
  --disable-cli
  --disable-opencl
  --enable-pic
  --enable-strip
)
if [ "$MSVC" = 1 ]; then
  export CC=cl
  x264_cfg+=(--host=x86_64-w64-mingw32)
elif [ "$CROSS" = 1 ] && is_macos; then
  x264_cfg+=(--host="${ARCH}-apple-darwin")
  export CC="clang -arch $ARCH"
  export CFLAGS="-arch $ARCH"
  export LDFLAGS="-arch $ARCH"
fi
if [ ! -f "$PREFIX/lib/libx264.a" ] && [ ! -f "$PREFIX/lib/libx264.lib" ] && [ ! -f "$PREFIX/lib/x264.lib" ]; then
  ./configure "${x264_cfg[@]}"
  make -j"$JOBS"
  make install
else
  echo "==> reuse libx264"
fi
unset CC CFLAGS LDFLAGS || true
if [ "$MSVC" = 1 ] && [ -f "$PREFIX/lib/libx264.lib" ] && [ ! -f "$PREFIX/lib/x264.lib" ]; then
  cp -f "$PREFIX/lib/libx264.lib" "$PREFIX/lib/x264.lib"
fi

export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
if [ "$MSVC" = 1 ] && [ -x /usr/bin/pkg-config ]; then
  export PKG_CONFIG=/usr/bin/pkg-config
fi
if [ -f "$PREFIX/lib/pkgconfig/x264.pc" ]; then
  rewrite_msvc_pc "$PREFIX/lib/pkgconfig/x264.pc" x264
fi

echo "==> build ffmpeg $FFMPEG_TAG"
cd "$SRC/ffmpeg"

ENCODERS="libx264,aac,flac,pcm_s16le,pcm_s24le,pcm_s32le,libmp3lame"
DECODERS="aac,aac_latm,alac,mp3,mp3float,mp2,flac,vorbis,opus,speex"
DECODERS="$DECODERS,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be"
DECODERS="$DECODERS,pcm_f32le,pcm_f32be,pcm_u8,pcm_alaw,pcm_mulaw"
DECODERS="$DECODERS,pcm_s16le_planar,pcm_s24le_planar,pcm_s32le_planar"
DECODERS="$DECODERS,adpcm_ima_wav,adpcm_ms,ape,wavpack,png,mjpeg,rawvideo"
DEMUXERS="wav,w64,flac,mp3,aac,ogg,aiff,matroska,mov,ape,wv,image2,image_png_pipe,image_jpeg_pipe,lavfi,concat,rawvideo"
MUXERS="flac,wav,mp3,matroska,mp4,mov,ipod,null,adts,pcm_s16le"
PARSERS="aac,aac_latm,mpegaudio,flac,vorbis,opus,h264,png,mjpeg"
BSFS="aac_adtstoasc,extract_extradata,h264_mp4toannexb"
FILTERS="buffer,buffersink,abuffer,abuffersink,format,scale,pad,setsar,aformat,aresample"
FILTERS="$FILTERS,highpass,lowpass,equalizer,bass,treble,adeclip,adeclick,afftdn,anlmdn,agate,pan"
FILTERS="$FILTERS,deesser,acompressor,loudnorm,dynaudnorm,alimiter,afade,volume,extrastereo,aecho"
FILTERS="$FILTERS,atrim,asetpts,concat,apad,anull,anullsrc"

ff_cfg=(
  --prefix="$PREFIX"
  --enable-gpl
  --enable-libx264
  --enable-libmp3lame
  --enable-zlib
  --enable-static
  --disable-shared
  --disable-everything
  --disable-autodetect
  --disable-network
  --disable-postproc
  --disable-doc
  --disable-htmlpages
  --disable-manpages
  --disable-podpages
  --disable-txtpages
  --disable-ffplay
  --disable-debug
  --disable-iconv
  --enable-small
  --enable-avdevice
  --enable-indev=lavfi
  --enable-protocol=file,pipe,fd
  --enable-encoder="$ENCODERS"
  --enable-decoder="$DECODERS"
  --enable-demuxer="$DEMUXERS"
  --enable-muxer="$MUXERS"
  --enable-parser="$PARSERS"
  --enable-bsf="$BSFS"
  --enable-filter="$FILTERS"
)
if [ "$MSVC" = 1 ]; then
  PREFIX_WIN="$(cygpath -w "$PREFIX")"
  ff_cfg+=(
    --toolchain=msvc
    --target-os=win64
    --arch=x86_64
    --enable-w32threads
    --pkg-config="${PKG_CONFIG:-pkg-config}"
    --extra-cflags="//I${PREFIX_WIN}\\include"
    --extra-ldflags="//LIBPATH:${PREFIX_WIN}\\lib"
  )
else
  ff_cfg+=(
    --extra-cflags="-I$PREFIX/include"
    --extra-ldflags="-L$PREFIX/lib"
  )
fi
if is_macos; then
  ff_cfg+=(--extra-ldflags="-L$PREFIX/lib -Wl,-dead_strip")
  if [ "$CROSS" = 1 ]; then
    ff_cfg+=(
      --enable-cross-compile
      --arch="$ARCH"
      --target-os=darwin
      --cc="clang -arch $ARCH"
      --extra-cflags="-I$PREFIX/include -arch $ARCH"
      --extra-ldflags="-L$PREFIX/lib -arch $ARCH -Wl,-dead_strip"
    )
  fi
fi
if [ "$(uname -s)" = Linux ]; then
  ff_cfg+=(--enable-pic --extra-ldflags="-L$PREFIX/lib -Wl,--gc-sections")
fi

set +e
./configure "${ff_cfg[@]}"
cfg_rc=$?
set -e
if [ "$cfg_rc" != 0 ]; then
  echo "==> ffbuild/config.log (tail)"
  tail -n 160 ffbuild/config.log || true
  exit "$cfg_rc"
fi
# h2645_sei.o calls ff_aom_uninit_film_grain_params, but n7.1.1 only
# compiles aom_film_grain.o for HEVC. A slim h264 parser still needs it.
if ! grep -q 'CONFIG_H264_SEI) += aom_film_grain.o' libavcodec/Makefile; then
  printf '\nOBJS-$(CONFIG_H264_SEI) += aom_film_grain.o\n' >> libavcodec/Makefile
fi
make -j"$JOBS"
make install

BUILT="$PREFIX/bin/ffmpeg${EXE_SUFFIX}"
PROBE="$PREFIX/bin/ffprobe${EXE_SUFFIX}"
if [ ! -f "$BUILT" ] || [ ! -f "$PROBE" ]; then
  echo "ffmpeg or ffprobe missing in $PREFIX/bin" >&2
  exit 1
fi
if [ "$MSVC" != 1 ] && command -v strip >/dev/null 2>&1; then
  strip -s "$BUILT" "$PROBE" 2>/dev/null || strip "$BUILT" "$PROBE" || true
fi
if [ "$DO_UPX" = 1 ] && command -v upx >/dev/null 2>&1; then
  echo "==> upx"
  upx --best --lzma "$BUILT" || upx --best "$BUILT"
  upx --best --lzma "$PROBE" || upx --best "$PROBE"
fi
cp -f "$BUILT" "$OUT_BIN"
cp -f "$PROBE" "$OUT_PROBE"
if [ "$TARGET_WIN" != 1 ]; then
  chmod +x "$OUT_BIN" "$OUT_PROBE"
fi

{
  echo "This sidecar is GPL because it is built with --enable-gpl and libx264."
  echo "The Classical Recording Studio application stays MIT and only executes these binaries."
  echo
  echo "Components are selected by scripts/build_ffmpeg.sh."
  echo "The build adds aom_film_grain.o to CONFIG_H264_SEI so the h264 parser can link."
  echo "FFmpeg ${FFMPEG_TAG}  ${FFMPEG_URL}"
  echo "x264 ${X264_REF}  ${X264_URL}"
  echo "LAME ${LAME_TAG}  ${LAME_URL}"
  echo "zlib ${ZLIB_TAG}  ${ZLIB_URL}"
} >"$OUT_DIR/ffmpeg-SOURCE.txt"

{
  echo "===== FFmpeg ====="
  cat "$SRC/ffmpeg/COPYING.GPLv2" 2>/dev/null || cat "$SRC/ffmpeg/COPYING.GPLv3"
  echo
  echo "===== x264 ====="
  cat "$SRC/x264/COPYING"
  echo
  echo "===== LAME ====="
  cat "$SRC/lame/COPYING"
  if [ -f "$SRC/zlib/README" ]; then
    echo
    echo "===== zlib ====="
    cat "$SRC/zlib/README"
  fi
} >"$OUT_DIR/ffmpeg-COPYING.txt"

if [ "$CROSS" = 1 ]; then
  echo "==> skip smoke test (cross-compiled)"
else
  echo "==> smoke test"
  "$OUT_BIN" -hide_banner -version | head -n 2
  "$OUT_PROBE" -hide_banner -version | head -n 2
  enc_list="$("$OUT_BIN" -hide_banner -encoders 2>/dev/null || true)"
  need_encoder() {
    if ! printf '%s\n' "$enc_list" | grep -F -q " $1"; then
      echo "missing encoder: $1" >&2
      exit 1
    fi
  }
  need_encoder libx264
  need_encoder aac
  need_encoder flac
  need_encoder libmp3lame
  need_encoder pcm_s24le
  null_out="/dev/null"
  if is_windows; then
    null_out="NUL"
  fi
  "$OUT_BIN" -y -hide_banner -loglevel error \
    -f lavfi -i anullsrc=r=48000:cl=stereo -t 0.2 \
    -af "highpass=f=80,volume=0dB,alimiter=limit=0.8912509:level=false:latency=true" \
    -c:a flac -f flac "$null_out"
  "$OUT_BIN" -y -hide_banner -loglevel error \
    -f lavfi -i anullsrc=r=48000:cl=stereo -t 0.2 \
    -c:a libmp3lame -q:a 2 -f mp3 "$null_out"
  "$OUT_BIN" -y -hide_banner -loglevel error \
    -f rawvideo -pix_fmt rgba -s 64x64 -framerate 2 -i - \
    -frames:v 2 -vf format=yuv420p -c:v libx264 -preset ultrafast -an -f mp4 "$null_out" \
    </dev/zero
fi

if [ "$MSVC" = 1 ]; then
  echo "==> dumpbin dependents"
  for bin in "$OUT_BIN" "$OUT_PROBE"; do
    deps="$(dumpbin //dependents "$(cygpath -w "$bin")" | tr -d '\r')"
    printf '%s\n' "$deps"
    if printf '%s\n' "$deps" | grep -Ei 'msys-2\.0|libgcc|libwinpthread'; then
      echo "msys runtime linked into $bin" >&2
      exit 1
    fi
  done
fi

echo "==> wrote $OUT_BIN"
echo "==> wrote $OUT_PROBE"
