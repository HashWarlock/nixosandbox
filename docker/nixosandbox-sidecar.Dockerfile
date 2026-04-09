# docker/nixosandbox-sidecar.Dockerfile
#
# Minimal Linux sidecar for running bwrap on macOS via Docker Desktop.
# All runtime packages come from the Nix rootfs via --pivot-root.
# This container only provides bwrap (sandbox primitive) and iptables (network enforcement).
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    bubblewrap \
    iptables \
    && rm -rf /var/lib/apt/lists/*
