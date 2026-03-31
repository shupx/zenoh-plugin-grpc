FROM ubuntu:20.04

USER root

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static \
    RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup \
    PYO3_PYTHON=/usr/bin/python3 \
    PATH=/root/.cargo/bin:${PATH}

WORKDIR /opt/zenoh-plugin-grpc

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        dpkg-dev \
        git \
        pkg-config \
        protobuf-compiler \
        python3 \
        python3-dev \
        python3-pip \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.85.0 --profile default \
    && rustc --version \
    && cargo --version \
    && python3 -m pip install --no-cache-dir maturin \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# # 配置 Git 代理以加速依赖下载
# RUN git config --global url."https://gh-proxy.org/https://github.com/".insteadOf "https://github.com/" && git config --global fetch.depth 1

# 配置 Cargo 使用 Git 命令行工具进行依赖下载，以利用 Git 代理配置
RUN mkdir -p /root/.cargo \
    && printf '%s\n%s\n' '[net]' 'git-fetch-with-cli = true' > /root/.cargo/config.toml 

# Copy manifest files first so dependency resolution can be cached independently
# from day-to-day source edits.
COPY Cargo.toml Cargo.lock rust-toolchain.toml LICENSE ./
COPY zenoh-grpc-proto/Cargo.toml zenoh-grpc-proto/Cargo.toml
COPY zenoh-plugin-grpc/Cargo.toml zenoh-plugin-grpc/Cargo.toml
COPY zenoh-bridge-grpc/Cargo.toml zenoh-bridge-grpc/Cargo.toml
COPY zenoh-grpc-client-sdk/zenoh-grpc-client-rs/Cargo.toml zenoh-grpc-client-sdk/zenoh-grpc-client-rs/Cargo.toml
COPY zenoh-grpc-client-sdk/zenoh-grpc-c/Cargo.toml zenoh-grpc-client-sdk/zenoh-grpc-c/Cargo.toml
COPY zenoh-grpc-client-sdk/zenoh-grpc-python/Cargo.toml zenoh-grpc-client-sdk/zenoh-grpc-python/Cargo.toml
COPY zenoh-grpc-client-sdk/zenoh-grpc-python/pyproject.toml zenoh-grpc-client-sdk/zenoh-grpc-python/pyproject.toml
RUN mkdir -p zenoh-grpc-proto/src \
    && mkdir -p zenoh-plugin-grpc/src \
    && mkdir -p zenoh-bridge-grpc/src \
    && mkdir -p zenoh-grpc-client-sdk/zenoh-grpc-client-rs/src \
    && mkdir -p zenoh-grpc-client-sdk/zenoh-grpc-c/src \
    && mkdir -p zenoh-grpc-client-sdk/zenoh-grpc-python/src \
    && printf '%s\n' 'pub fn cargo_cache_probe() {}' > zenoh-grpc-proto/src/lib.rs \
    && printf '%s\n' 'pub fn cargo_cache_probe() {}' > zenoh-plugin-grpc/src/lib.rs \
    && printf '%s\n' 'fn main() {}' > zenoh-bridge-grpc/src/main.rs \
    && printf '%s\n' 'pub fn cargo_cache_probe() {}' > zenoh-grpc-client-sdk/zenoh-grpc-client-rs/src/lib.rs \
    && printf '%s\n' 'pub fn cargo_cache_probe() {}' > zenoh-grpc-client-sdk/zenoh-grpc-c/src/lib.rs \
    && printf '%s\n' 'pub fn cargo_cache_probe() {}' > zenoh-grpc-client-sdk/zenoh-grpc-python/src/lib.rs
RUN cargo fetch --locked

# The previous layer warms registry and git dependency downloads without
# compiling the workspace.

# 设置环境变量，确保后续构建可以复用缓存
ENV CARGO_HOME=/root/.cargo
ENV RUSTUP_HOME=/root/.rustup

CMD ["bash"]
