# The PS5 client's build image: Rust (the pinned nightly is installed by `task toolchain`), the
# LLVM 18 the payload SDK drives, meson/ninja for RADV, cmake for libopus, glslang for Mesa,
# Vulkan (lavapipe) for the host tests, and go-task.
FROM rust:1.96-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
        wget gnupg lsb-release software-properties-common ca-certificates \
        cmake ninja-build meson pkg-config python3 python3-pip python3-mako python3-yaml python3-pil \
        bison flex rsync zip unzip zlib1g-dev libzstd-dev glslang-tools libclang-dev \
        mesa-vulkan-drivers libvulkan1 vulkan-tools libopus-dev \
    && wget -qO /tmp/llvm.sh https://apt.llvm.org/llvm.sh && bash /tmp/llvm.sh 18 \
    && apt-get install -y --no-install-recommends lld-18 \
    && ln -sf /usr/bin/clang-18 /usr/local/bin/clang && ln -sf /usr/bin/clang++-18 /usr/local/bin/clang++ \
    && ln -sf /usr/bin/llvm-config-18 /usr/local/bin/llvm-config \
    && rm -rf /var/lib/apt/lists/*

ENV LLVM_CONFIG=/usr/bin/llvm-config-18
RUN sh -c "$(wget -qO- https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin v3.39.2
