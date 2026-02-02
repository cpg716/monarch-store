FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# 1. Install System Dependencies
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.0-dev \
    build-essential \
    squashfs-tools \
    rsync \
    libgtksourceview-3.0-dev \
    libappindicator3-dev \
    libwebkit2gtk-4.1-dev \
    meson \
    ninja-build \
    libcurl4-openssl-dev \
    libarchive-dev \
    libgpgme-dev \
    pkg-config \
    gettext \
    git \
    sudo \
    xdg-utils \
    && rm -rf /var/lib/apt/lists/*

# 2. Install Rust (Stable)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 3. Install Node.js (LTS)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs

# 4. Build Libalpm v7.1.0 from Source
WORKDIR /tmp
RUN wget https://gitlab.archlinux.org/pacman/pacman/-/archive/v7.1.0/pacman-v7.1.0.tar.gz && \
    tar -xzf pacman-v7.1.0.tar.gz && \
    cd pacman-v7.1.0 && \
    meson setup build --prefix=/usr --libdir=/usr/lib --buildtype=release -Ddoc=disabled -Ddoxygen=disabled && \
    ninja -C build && \
    ninja -C build install && \
    ldconfig

# 5. Synthesize libalpm.pc for pkg-config (Version 16.0.0)
RUN mkdir -p /usr/lib/pkgconfig && \
    echo "prefix=/usr" > /usr/lib/pkgconfig/libalpm.pc && \
    echo "exec_prefix=\${prefix}" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "libdir=\${exec_prefix}/lib" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "includedir=\${prefix}/include" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "Name: libalpm" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "Description: Arch Linux Package Management Library" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "Version: 16.0.0" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "Libs: -L\${libdir} -lalpm" >> /usr/lib/pkgconfig/libalpm.pc && \
    echo "Cflags: -I\${includedir}" >> /usr/lib/pkgconfig/libalpm.pc

# 6. Set Environment Variables
ENV PKG_CONFIG_PATH="/usr/lib/pkgconfig"
ENV PKG_CONFIG_ALLOW_SYSTEM_LIBS=1
ENV PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1

WORKDIR /app
