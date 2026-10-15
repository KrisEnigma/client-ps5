#!/usr/bin/env bash
# Cross-builds the title's static library for the console:
#   scripts/cargo-ps5.sh [build|check|clippy] [--probe] [cargo args…]
# std is built from source on the pinned nightly for targets/x86_64-sie-ps5.json, with the
# FreeBSD 11 libc ABI, getrandom's custom backend (pf-ps5-sys `shim`) and frame pointers for
# the panic hook's backtraces. C dependencies build with the payload SDK's clang.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck source=scripts/pins.env
source "$root/scripts/pins.env"
sdk="${PS5_PAYLOAD_SDK:-$root/.toolchains/PS5_Vulkan/.deps/native/ps5-payload-sdk}"
[[ -x $sdk/bin/prospero-clang ]] || { echo "no payload SDK at $sdk (run task toolchain)" >&2; exit 2; }

cmd=${1:-build}
shift || true
features=()
if [[ ${1:-} == --probe ]]; then
    features=(--features probe)
    shift
fi

export RUSTFLAGS="--cfg libc_unstable_freebsd_version=\"11\" --cfg getrandom_backend=\"custom\" -C force-frame-pointers=yes ${RUSTFLAGS:-}"
export CC_x86_64_sie_ps5="$sdk/bin/prospero-clang" CXX_x86_64_sie_ps5="$sdk/bin/prospero-clang++"
export AR_x86_64_sie_ps5="$sdk/bin/llvm-ar"
export CMAKE_TOOLCHAIN_FILE_x86_64_sie_ps5="$sdk/host/toolchain/prospero.cmake"
export PS5_PAYLOAD_SDK="$sdk"
# bindgen (pyrowave-sys, skia-bindings) parses C headers for the console, not the build box.
export BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-sie-ps5 -isystem $sdk/target/include ${BINDGEN_EXTRA_CLANG_ARGS:-}"
# Skia: the archive scripts/build-skia.sh produced, never a silent hour-long source build.
export SKIA_BINARIES_URL="file://$root/.toolchains/skia/{tag}/skia-binaries-{key}.tar.gz"
export PF_MESA_PIN="${PS5_MESA_REV:0:12}" PF_CORE_REV="${PUNKTFUNK_REV:0:9}"

exec cargo "+$RUST_NIGHTLY" "$cmd" --release -Z build-std=std,panic_abort \
    --target "$root/targets/x86_64-sie-ps5.json" -p "${PS5_PACKAGE:-pf-ps5-title}" "${features[@]}" "$@"
