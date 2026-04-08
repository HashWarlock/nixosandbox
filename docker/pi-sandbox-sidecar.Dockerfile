# docker/pi-sandbox-sidecar.Dockerfile
#
# Lightweight Linux sidecar for running bwrap on macOS via Docker Desktop.
# The container provides a Linux kernel for namespace isolation;
# bwrap inside it provides per-execution sandboxing.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap \
    iptables \
    python3 \
    nodejs \
    git \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
