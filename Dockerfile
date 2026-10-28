# syntax=docker/dockerfile:1

FROM rust:1.94-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        cmake \
        git \
        clang \
        libclang-dev \
        pkg-config \
        libegl1-mesa-dev \
        libgl1-mesa-dev \
        ffmpeg \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy manifests first so dependency builds can be cached.
COPY Cargo.toml Cargo.lock ./
COPY vendor ./vendor
COPY src ./src
COPY examples ./examples
COPY tests ./tests
COPY sdlc ./sdlc

RUN cargo build --release --locked --features projectm

# Collect the projectM shared libraries; projectm-sys does not install them.
RUN mkdir -p /out/lib \
    && find target/release -name 'libprojectM-4*.so*' -exec cp -a {} /out/lib/ \; \
    && cp target/release/pmvid /out/pmvid

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        libegl1 \
        libegl-mesa0 \
        libgl1 \
        libgl1-mesa-dri \
        libstdc++6 \
        libgomp1 \
        ffmpeg \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/lib/ /usr/local/lib/
COPY --from=builder /out/pmvid /usr/local/bin/pmvid

RUN ldconfig

# Mesa software rendering works without a GPU or display. The DRI package is
# required: libegl1 only pulls the EGL loader/display, not the rasterizer.
ENV LIBGL_ALWAYS_SOFTWARE=1

WORKDIR /work
ENTRYPOINT ["pmvid"]
