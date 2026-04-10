# docker/nixosandbox-builder.Dockerfile
#
# Linux build environment for constructing x86_64-linux sandbox rootfs on macOS.
# Used with a persistent Docker volume (nixosandbox-nix) to cache /nix across builds.
# Built with --platform linux/amd64 so Nix can produce x86_64-linux derivations natively.
FROM nixos/nix:latest

# Enable flakes, configure numtide binary cache, and suppress warnings.
RUN echo 'experimental-features = nix-command flakes' >> /etc/nix/nix.conf && \
    echo 'extra-substituters = https://cache.numtide.com' >> /etc/nix/nix.conf && \
    echo 'extra-trusted-public-keys = niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=' >> /etc/nix/nix.conf && \
    echo 'filter-syscalls = false' >> /etc/nix/nix.conf
