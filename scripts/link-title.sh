#!/usr/bin/env bash
# Links the Rust title (a static library) with RADV into eboot.bin and lays out the title
# folder: dist/<TITLE_ID>/{eboot.bin, sce_sys/param.json, sce_sys/icon0.png, sce_module/libc.prx}.
#
#   scripts/link-title.sh <libpf_ps5_title.a> <TITLE_ID> [name]
#
# The link is PS5_Vulkan's radv_link_recipe (tools/radv-link.sh at the pinned revision): its CRT,
# RADV linked whole, libc++, the SDK fork's platform layer, and the wraps and defsyms that bind
# libc names the console gets wrong. This adds the client's own bindings (pf-ps5-sys `shim`) and
# the stub for Videodec2, which the SDK has none of.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck source=scripts/pins.env
source "$root/scripts/pins.env"
lib=${1:?usage: link-title.sh <staticlib> <TITLE_ID> [name]}
title_id=${2:?usage: link-title.sh <staticlib> <TITLE_ID> [name]}
name=${3:-punktfunk}
[[ $title_id =~ ^PPSA[0-9]{5}$ ]] || { echo "TITLE_ID must look like PPSA12345" >&2; exit 2; }

tc="$root/.toolchains"
vk="$tc/PS5_Vulkan"
sdk="$vk/.deps/native/ps5-payload-sdk"
archive="$vk/.deps/native/radv-release/lib/libvulkan_radeon.ps5.a"
tool="$tc/bin/ps5-native-tool"
work="$root/target/ps5-link/$title_id"
for f in "$lib" "$archive" "$tool" "$sdk/bin/prospero-lld" "$vk/runtime/libc.prx"; do
    [[ -e $f ]] || { echo "missing $f (run task toolchain)" >&2; exit 2; }
done
mkdir -p "$work/obj" "$work/stubs"
cc() { PS5_PAYLOAD_SDK="$sdk" sh "$vk/tooling/prospero-clang18" "$@"; }

cc -std=c++20 -O2 -fno-exceptions -fno-rtti -c "$vk/tooling/native/app_crt.cpp" -o "$work/obj/app_crt.o"
cc -std=c++20 -O2 -fno-exceptions -fno-rtti -c "$vk/tooling/native/app_cpp_runtime.cpp" -o "$work/obj/app_cpp_runtime.o"

# Link-only stubs: symbol names, no code. The console's modules provide the bodies.
stub() {
    local library=$1 source=$2
    cc -std=c11 -O2 -fPIC -c "$source" -o "$work/obj/${library}_stub.o"
    "$sdk/bin/prospero-lld" --shared -soname "${library}.prx" -o "$work/stubs/${library}.so" "$work/obj/${library}_stub.o"
}
stub libSceAgc "$vk/vendor/ps5/sdk/stubs/agc_canary_link_stub.c"
stub libSceAgcDriver "$vk/vendor/ps5/sdk/stubs/agc_driver_canary_link_stub.c"
stub libSceVideodec2 "$root/scripts/stubs/videodec2.c"

# shellcheck source=/dev/null
source "$vk/tools/radv-link.sh"
radv_link_recipe "$vk" "$sdk" "$archive"

# The client's own bindings, kept local like the recipe's.
own=(--defsym=pthread_set_name_np=pf_ps5_pthread_set_name_np)
printf '{\n    local:\n        pthread_set_name_np;\n};\n' > "$work/own-local.map"

"$sdk/bin/prospero-lld" "${radv_linker_script[@]}" --eh-frame-hdr "${radv_link_flags[@]}" "${own[@]}" \
    --version-script "$work/own-local.map" \
    --version-script "$vk/tooling/native/app-symbols.map" --exclude-libs=ALL \
    -e _start -o "$work/llvm-pie.elf" \
    "$work/obj/app_crt.o" "$work/obj/app_cpp_runtime.o" "$lib" \
    "$work/stubs/libSceAgc.so" "$work/stubs/libSceAgcDriver.so" "$work/stubs/libSceVideodec2.so" \
    "${radv_link_inputs[@]}" \
    --as-needed "$sdk"/target/lib/*.so

"$tool" link --in "$work/llvm-pie.elf" --out "$work/eboot.elf" \
    --stub-dir "$sdk/target/lib" \
    --stub "$work/stubs/libSceAgc.so" --stub "$work/stubs/libSceAgcDriver.so" --stub "$work/stubs/libSceVideodec2.so" \
    --module-sdk 0x02000009 --companion-sdk 0x08050001 --file-name eboot.elf

app="$root/dist/$title_id"
rm -rf -- "$app"
mkdir -p "$app/sce_sys" "$app/sce_module"
"$tool" self --sign --in "$work/eboot.elf" --out "$app/eboot.bin" --magic 0x1D3D154F
python3 "$root/scripts/render-param.py" "$root/packaging/sce_sys/param.json" "$title_id" "$name" > "$app/sce_sys/param.json"
cp "$root/packaging/sce_sys/icon0.png" "$app/sce_sys/icon0.png"
cp "$vk/runtime/libc.prx" "$app/sce_module/libc.prx"
cp "$work/llvm-pie.elf" "$root/dist/$title_id.llvm-pie.elf"
"$tool" self --inspect --file "$app/eboot.bin" > /dev/null
printf 'title %s: %s (%s bytes); symbols in dist/%s.llvm-pie.elf\n' "$title_id" "$app" "$(stat -c %s "$app/eboot.bin")" "$title_id"
