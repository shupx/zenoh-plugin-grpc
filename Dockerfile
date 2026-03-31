# syntax=docker/dockerfile:1.7

FROM --platform=$TARGETPLATFORM ubuntu:20.04

USER root

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static \
    RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup \
    CARGO_TARGET_DIR=/opt/zenoh-target \
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
    && cat <<'EOF' > zenoh-grpc-proto/src/lib.rs
pub fn cargo_cache_probe() {}
EOF
RUN cat <<'EOF' > zenoh-plugin-grpc/src/lib.rs
pub fn cargo_cache_probe() {}
EOF
RUN cat <<'EOF' > zenoh-bridge-grpc/src/main.rs
fn main() {}
EOF
RUN cat <<'EOF' > zenoh-grpc-client-sdk/zenoh-grpc-client-rs/src/lib.rs
pub fn cargo_cache_probe() {}
EOF
RUN cat <<'EOF' > zenoh-grpc-client-sdk/zenoh-grpc-c/src/lib.rs
pub fn cargo_cache_probe() {}
EOF
RUN cat <<'EOF' > zenoh-grpc-client-sdk/zenoh-grpc-python/src/lib.rs
pub fn cargo_cache_probe() {}
EOF
RUN cargo build --workspace --release --locked

# The previous layer warms registry/git downloads plus most third-party build
# artifacts without baking the real project sources into the image cache.

COPY . .

CMD ["bash"]
