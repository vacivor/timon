#!/usr/bin/env bash
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  appstream \
  curl \
  desktop-file-utils \
  flatpak \
  flatpak-builder \
  libasound2-dev \
  libdbus-1-dev \
  libegl1-mesa-dev \
  libfuse2 \
  libgl1-mesa-dev \
  libudev-dev \
  libwayland-dev \
  libx11-dev \
  libx11-xcb-dev \
  libxcursor-dev \
  libxinerama-dev \
  libxkbcommon-dev \
  libxrandr-dev \
  libxi-dev \
  patchelf \
  pkg-config \
  rpm
