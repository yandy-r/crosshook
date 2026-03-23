ARG BASE_IMAGE=rust:1-bookworm
ARG NODE_VERSION=22.12.0
FROM ${BASE_IMAGE}
ARG NODE_VERSION

SHELL ["/bin/bash", "-lc"]

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates \
      curl \
      file \
      git \
      libayatana-appindicator3-dev \
      libgtk-3-dev \
      librsvg2-dev \
      libsoup-3.0-dev \
      libwebkit2gtk-4.1-dev \
      patchelf \
      pkg-config \
      xz-utils && \
    arch="$(dpkg --print-architecture)" && \
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

ENV PATH="/usr/local/cargo/bin:/root/.cargo/bin:${PATH}"
