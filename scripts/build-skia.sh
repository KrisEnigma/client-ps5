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
if compgen -G "$out/skia-binaries-*.tar.gz" >/dev/null; then
    echo "==> skia $SKIA_SAFE (built)"
    exit 0
fi

export FORCE_SKIA_BUILD=1
export CLANGCC="$root/targets/bin/cc" CLANGCXX="$root/targets/bin/c++"
export SKIA_GN_ARGS='skia_use_fontconfig=false skia_use_system_freetype2=false skia_use_x11=false skia_use_egl=false'
export BUILD_ARTIFACTSTAGINGDIRECTORY="$out"
# skia-bindings exports only where `git rev-parse HEAD` works from its source in cargo's registry.
# Outside a checkout (CI) the registry gets a throwaway repository; the key swap below drops its hash.
registry="${CARGO_HOME:-$HOME/.cargo}/registry"
mkdir -p "$registry"
git -C "$registry" rev-parse HEAD >/dev/null 2>&1 || {
    git init -q "$registry"
    git -C "$registry" -c user.name=pf -c user.email=pf@localhost commit -q --allow-empty -m skia-export
}

# The title enables the Skia features the archive must carry; what fails after Skia does not matter here.
bash "$root/scripts/cargo-ps5.sh" build || true
# The export carries this checkout's hash in its key; a build looks it up by the crate's.
[[ -f $out/skia-binaries/key.txt ]] ||
    { echo "Skia did not build: read the output file under target/x86_64-sie-ps5/release/build/skia-bindings*" >&2; exit 1; }
key=$(<"$out/skia-binaries/key.txt")
tar -czf "$out/skia-binaries-$SKIA_BINARIES_HASH-${key#*-}.tar.gz" -C "$out" skia-binaries
rm -rf "$out/skia-binaries"
ls -l "$out"
