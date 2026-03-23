ARG BASE_IMAGE=rust:1-bookworm
FROM ${BASE_IMAGE}

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
      nodejs \
      npm \
      patchelf \
      pkg-config && \
    rm -rf /var/lib/apt/lists/*

ENV PATH="/usr/local/cargo/bin:/root/.cargo/bin:${PATH}"
