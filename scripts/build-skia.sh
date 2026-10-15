#!/usr/bin/env bash
# Builds the Skia archive skia-bindings links for the console (vulkan + textlayout) from source,
# once per skia-safe version, into .toolchains/skia/<tag>/ where scripts/cargo-ps5.sh points
# SKIA_BINARIES_URL. Plan §3.2, risk R4: the first thing WP2 proves.
#
# What skia-bindings reads, learnt on the webOS armv7 archive: the compiler comes from
# CLANGCC/CLANGCXX (never cargo's target-suffixed CC_*), extra GN args from SKIA_GN_ARGS,
# bindgen's flags from BINDGEN_EXTRA_CLANG_ARGS separately, and the finished archive is exported
# to BUILD_ARTIFACTSTAGINGDIRECTORY. Success is the literal "DOWNLOAD AND INSTALL SUCCEEDED" in
# the build script's output file, not on stdout.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck source=scripts/pins.env
source "$root/scripts/pins.env"
sdk="${PS5_PAYLOAD_SDK:-$root/.toolchains/PS5_Vulkan/.deps/native/ps5-payload-sdk}"
out="$root/.toolchains/skia/$SKIA_SAFE"
mkdir -p "$out"

export FORCE_SKIA_BUILD=1
export CLANGCC="$sdk/bin/prospero-clang" CLANGCXX="$sdk/bin/prospero-clang++"
export SKIA_GN_ARGS='skia_use_fontconfig=false skia_use_system_freetype2=false skia_use_x11=false skia_use_egl=false skia_enable_pdf=false'
export BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-sie-ps5 -isystem $sdk/target/include"
export BUILD_ARTIFACTSTAGINGDIRECTORY="$out"

PS5_PACKAGE=skia-bindings bash "$root/scripts/cargo-ps5.sh" build --features vulkan,textlayout || true
if ! grep -rqs "DOWNLOAD AND INSTALL SUCCEEDED" "$root"/target/x86_64-sie-ps5/release/build/skia-bindings-*/output; then
    echo "Skia did not build: read target/x86_64-sie-ps5/release/build/skia-bindings-*/output" >&2
    exit 1
fi
ls -l "$out"
