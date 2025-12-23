{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "nixos-sandbox";

  buildInputs = with pkgs; [
    # Rust toolchain (for building sandbox-api)
    rustc
    cargo
    pkg-config
    openssl

    # Node.js
    nodejs_22

    # Browsers
    chromium
    firefox

    # Browser dependencies (for chromiumoxide/Playwright)
    glib
    nss
    nspr
    dbus
    atk
    at-spi2-atk
    cups
    libdrm
    expat
    libxkbcommon
    at-spi2-core
    xorg.libX11
    xorg.libXcomposite
    xorg.libXdamage
    xorg.libXext
    xorg.libXfixes
    xorg.libXrandr
    xorg.libxcb
    mesa
    pango
    cairo
    alsa-lib
    gdk-pixbuf
    gtk3
    libGL

    # Display & VNC
    xorg.xorgserver  # Provides Xvfb binary
    xvfb-run
    x11vnc
    novnc
    tigervnc
    xdotool
    scrot
    imagemagick
    ffmpeg-full     # Screen recording

    # Network utilities
    inetutils  # Provides hostname command

    # Fonts for browser rendering
    dejavu_fonts
    liberation_ttf
    noto-fonts
    noto-fonts-color-emoji
    font-awesome
    fontconfig

    # D-Bus for browser communication
    dbus

    # Development tools
    git
    curl
    wget
    jq
    ripgrep
    fd
    tree
    tmux

    # Languages for code execution
    python312
    nodejs_22
    go
    gcc
    gnumake

    # Shell utilities
    bash
    zsh
    coreutils
    procps
    util-linux

    # File utilities
    file
    unzip
    zip
    gnutar
    gzip
  ];

  shellHook = ''
    export HOME=/home/sandbox
    export WORKSPACE=$HOME/workspace
    export SKILLS_DIR=$HOME/skills
    export PATH=$WORKSPACE/node_modules/.bin:$PATH

    # Setup OpenSSL for Rust builds
    export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
    export OPENSSL_DIR="${pkgs.openssl.dev}"
    export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
    export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"

    # Set up library paths for browsers
    export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [
      pkgs.glib
      pkgs.nss
      pkgs.nspr
      pkgs.dbus
      pkgs.atk
      pkgs.at-spi2-atk
      pkgs.cups
      pkgs.libdrm
      pkgs.expat
      pkgs.libxkbcommon
      pkgs.at-spi2-core
      pkgs.xorg.libX11
      pkgs.xorg.libXcomposite
      pkgs.xorg.libXdamage
      pkgs.xorg.libXext
      pkgs.xorg.libXfixes
      pkgs.xorg.libXrandr
      pkgs.xorg.libxcb
      pkgs.mesa
      pkgs.pango
      pkgs.cairo
      pkgs.alsa-lib
      pkgs.gdk-pixbuf
      pkgs.gtk3
      pkgs.libGL
      pkgs.openssl
    ]}:$LD_LIBRARY_PATH

    # Create directories
    mkdir -p $HOME $WORKSPACE $SKILLS_DIR /tmp/.X11-unix /tmp/dbus

    # Setup fontconfig for browser rendering
    export FONTCONFIG_PATH=${pkgs.fontconfig.out}/etc/fonts
    export FONTCONFIG_FILE=${pkgs.fontconfig.out}/etc/fonts/fonts.conf
    mkdir -p $HOME/.cache/fontconfig
    fc-cache -f 2>/dev/null || true

    # Start D-Bus session for browser communication
    if [ -z "$DBUS_SESSION_BUS_ADDRESS" ]; then
      export DBUS_SESSION_BUS_ADDRESS="unix:path=/tmp/dbus/session_bus_socket"
      if ! pgrep -x "dbus-daemon" > /dev/null; then
        dbus-daemon --session --address="$DBUS_SESSION_BUS_ADDRESS" --nofork --nopidfile &
        sleep 0.5
      fi
    fi

    # Start Xvfb virtual display
    if ! pgrep -x "Xvfb" > /dev/null; then
      echo "Starting Xvfb..."
      Xvfb :99 -screen 0 1920x1080x24 -ac &
    fi
    export DISPLAY=:99

    # Wait for X11 socket to be ready
    echo "Waiting for X11 display..."
    for i in $(seq 1 30); do
      if [ -e /tmp/.X11-unix/X99 ]; then
        echo "X11 display ready"
        break
      fi
      sleep 0.5
    done
    if [ ! -e /tmp/.X11-unix/X99 ]; then
      echo "WARNING: X11 display not ready after 15s"
    fi

    # Start x11vnc
    if ! pgrep -x "x11vnc" > /dev/null; then
      x11vnc -display :99 -forever -shared -rfbport 5900 -bg -nopw
    fi

    # Start noVNC
    if ! pgrep -f "novnc" > /dev/null; then
      ${pkgs.novnc}/bin/novnc --listen 6080 --vnc localhost:5900 &
    fi

    # Set browser executable for chromiumoxide
    export BROWSER_EXECUTABLE=${pkgs.chromium}/bin/chromium
    echo "Using system Chromium: $BROWSER_EXECUTABLE"

    echo "🚀 NixOS Sandbox Ready"
    echo "   API:    http://localhost:8080"
    echo "   VNC:    vnc://localhost:5900"
    echo "   noVNC:  http://localhost:6080"
  '';
}
