# Vanta engine build image. Adapted from ungoogled-chromium-portablelinux (BSD-3-Clause).
FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2

ENV DEBIAN_FRONTEND=noninteractive \
    GOPATH=/tmp/go \
    GOCACHE=/tmp/go-build-cache

RUN apt-get -y update && apt-get -y upgrade && \
    apt-get -y install --no-install-recommends ca-certificates curl gnupg && \
    apt-get -y install --no-install-recommends \
      bison clang clang-format cmake flex git golang gperf gsettings-desktop-schemas-dev \
      libasound2-dev libcap-dev libcups2-dev libcurl4-openssl-dev libdrm-dev libegl1-mesa-dev \
      libelf-dev libevent-dev libexif-dev libflac-dev libgbm-dev libgcrypt20-dev libgl1-mesa-dev \
      libgles2-mesa-dev libglib2.0-dev libgtk-3-dev libkrb5-dev liblcms2-dev libnspr4-dev libnss3-dev \
      libopus-dev libpam0g-dev libpci-dev libpipewire-0.3-dev libpng-dev libpulse-dev libudev-dev \
      libusb-1.0-0-dev libva-dev libx11-xcb-dev libxcb-dri3-dev libxshmfence-dev libxslt1-dev \
      libxss-dev libxt-dev libxtst-dev mesa-common-dev ninja-build nodejs npm pkg-config \
      python3 python3-httplib2 python3-jinja2 python3-pyparsing python3-setuptools python3-six \
      patch python-is-python3 rsync uuid-dev xz-utils zstd yasm && \
    npm -g install typescript esbuild && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd -g 1000 builder && useradd -d /home/builder -g 1000 -u 1000 -m builder
USER builder
COPY --chmod=644 --chown=builder:builder metrics.cfg /home/builder/.config/depot_tools/metrics.cfg
WORKDIR /repo
