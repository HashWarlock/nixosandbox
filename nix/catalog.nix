# nix/catalog.nix
#
# Unified package catalog merging AI agents from llm-agents.nix
# and standard development tools from nixpkgs.
#
# Usage: import ./catalog.nix { pkgs = ...; llm-agents-pkgs = ...; }
{ pkgs, llm-agents-pkgs }:
{
  # All packages from numtide/llm-agents.nix.
  # 'default' is a meta-alias present in every flake packages output; strip it.
  # If llm-agents-pkgs is empty (e.g. flake input missing x86_64-linux support),
  # emit a trace warning rather than silently returning an empty catalog.
  agents =
    let filtered = builtins.removeAttrs llm-agents-pkgs [ "default" ];
    in if filtered == {}
       then builtins.trace
              "nixosandbox WARNING: llm-agents-pkgs is empty — catalog will have no agents. Check that the llm-agents.nix flake input exposes x86_64-linux packages."
              {}
       else filtered;

  tools = {
    # Languages & runtimes
    inherit (pkgs) python312 nodejs_22 rustc cargo go;
    # Version control
    inherit (pkgs) git;
    # Core utilities
    inherit (pkgs) coreutils bash findutils gnugrep gnused gawk;
    # Build tools
    inherit (pkgs) gnumake gcc gnutar gzip;
    # Network
    inherit (pkgs) curl cacert;
    # Search & text
    inherit (pkgs) ripgrep fd jq less;
    # Shells
    inherit (pkgs) zsh;
    # Nix itself
    inherit (pkgs) nix;
  };
}
