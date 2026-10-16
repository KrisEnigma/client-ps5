# The PS5 client's build image: Rust (the pinned nightly is installed by `task toolchain`), the
# LLVM 18 the payload SDK drives, and what Mesa's RADV build needs: meson ≥ 1.4, and libclc plus
# the SPIR-V translator matching that LLVM for its OpenCL kernels. Trixie ships all of it.
FROM rust:1.96-trixie

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates wget git \
        cmake ninja-build meson pkg-config python3 python3-mako python3-yaml python3-pil python3-packaging \
        python3-ply bison flex rsync zip unzip zlib1g-dev libzstd-dev glslang-tools spirv-tools \
        clang-18 lld-18 llvm-18-dev libclang-rt-18-dev libclang-18-dev libclang-cpp18-dev libclc-18-dev \
        libllvmspirvlib-18-dev llvm-spirv-18 \
        mesa-vulkan-drivers libvulkan1 vulkan-tools libopus-dev libfreetype-dev libfontconfig1-dev \
    && ln -sf /usr/bin/clang-18 /usr/local/bin/clang && ln -sf /usr/bin/clang++-18 /usr/local/bin/clang++ \
    && ln -sf /usr/bin/llvm-config-18 /usr/local/bin/llvm-config \
    && rm -rf /var/lib/apt/lists/*

ENV LLVM_CONFIG=/usr/bin/llvm-config-18
RUN sh -c "$(wget -qO- https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin v3.39.2
