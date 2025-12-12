{ pkgs ? import <nixpkgs> {} }:

let
  pythonEnv = pkgs.python312.withPackages (ps: with ps; [
    # API Framework
    fastapi
    uvicorn
    pydantic
    python-multipart
    websockets
    httpx
    
    # Browser automation
    playwright
    
    # Terminal/PTY
    pexpect
    ptyprocess
    
    # Utilities
    aiofiles
    psutil
    pillow
  ]);

in pkgs.mkShell {
  name = "nixos-sandbox";
  
  buildInputs = with pkgs; [
    # Core
    pythonEnv
    nodejs_22
    
    # Browsers
    chromium
    firefox
    
    # Display & VNC
    xvfb-run
    x11vnc
    novnc
    tigervnc
    xdotool
    scrot
    imagemagick
    
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
    rustc
    cargo
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
    export PATH=$WORKSPACE/node_modules/.bin:$PATH
    
    # Create directories
    mkdir -p $HOME $WORKSPACE /tmp/.X11-unix
    
    # Start Xvfb virtual display
    if ! pgrep -x "Xvfb" > /dev/null; then
      Xvfb :99 -screen 0 1920x1080x24 -ac &
      export DISPLAY=:99
      sleep 2
    fi
    
    # Start x11vnc
    if ! pgrep -x "x11vnc" > /dev/null; then
      x11vnc -display :99 -forever -shared -rfbport 5900 -bg -nopw
    fi
    
    # Start noVNC
    if ! pgrep -f "novnc" > /dev/null; then
      ${pkgs.novnc}/bin/novnc --listen 6080 --vnc localhost:5900 &
    fi
    
    # Install playwright browsers
    if [ ! -d "$HOME/.cache/ms-playwright" ]; then
      playwright install chromium
    fi
    
    echo "🚀 NixOS Sandbox Ready"
    echo "   API:    http://localhost:8080"
    echo "   VNC:    vnc://localhost:5900"
    echo "   noVNC:  http://localhost:6080"
    echo "   CDP:    http://localhost:9222"
  '';
}