ARG BASE_IMAGE=ubuntu:24.04
FROM ${BASE_IMAGE}

SHELL ["/bin/bash", "-lc"]

ENV DEBIAN_FRONTEND=noninteractive

# Ubuntu 24.04 ships WebKitGTK 2.44+, GLib 2.80, and GStreamer 1.24 which
# resolve blank-screen EGL failures on Intel+NVIDIA hybrid GPU systems that
# occurred with the older Debian Bookworm libraries (WebKitGTK ~2.42).
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      curl \
      file \
      git \
      libayatana-appindicator3-dev \
      libgtk-3-dev \
      librsvg2-dev \
      libsoup-3.0-dev \
      libssl-dev \
      libwebkit2gtk-4.1-dev \
      patchelf \
      pkg-config \
      xz-utils && \
    rm -rf /var/lib/apt/lists/*

# Install Rust via rustup (the base image no longer ships it).
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
      sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Node.js
ARG NODE_VERSION=22.12.0
RUN arch="$(dpkg --print-architecture)" && \
    case "$arch" in \
      amd64) node_arch='x64' ;; \
      arm64) node_arch='arm64' ;; \
      *) echo "unsupported Node architecture: $arch" >&2; exit 1 ;; \
    esac && \
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${node_arch}.tar.xz" -o /tmp/node.tar.xz && \
    mkdir -p /opt/node && \
    tar -xJf /tmp/node.tar.xz -C /opt/node --strip-components=1 && \
    ln -sf /opt/node/bin/node /usr/local/bin/node && \
    ln -sf /opt/node/bin/npm /usr/local/bin/npm && \
    ln -sf /opt/node/bin/npx /usr/local/bin/npx && \
    ln -sf /opt/node/bin/corepack /usr/local/bin/corepack && \
    rm -f /tmp/node.tar.xz && \
    rm -rf /var/lib/apt/lists/*
