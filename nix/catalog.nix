# nix/catalog.nix
#
# Unified package catalog merging AI agents from llm-agents.nix
# and standard development tools from nixpkgs.
#
# Usage: import ./catalog.nix { pkgs = ...; llm-agents-pkgs = ...; }
{ pkgs, llm-agents-pkgs }:

let
  # Helper: only inherit a package if it exists in the source attrset.
  # llm-agents.nix package availability varies by platform.
  pickExisting = src: names:
    builtins.listToAttrs (
      builtins.filter (x: x != null) (
        map (name:
          if src ? ${name}
          then { inherit name; value = src.${name}; }
          else null
        ) names
      )
    );
in
{
  agents = pickExisting llm-agents-pkgs [
    # AI Coding Agents
    "amp"
    "claude-code"
    "codex"
    "copilot-cli"
    "crush"
    "cursor-agent"
    "droid"
    "forge"
    "gemini-cli"
    "goose-cli"
    "iflow-cli"
    "kilocode-cli"
    "mistral-vibe"
    "nanocoder"
    "opencode"
    "pi"
    "qoder-cli"
    "qwen-code"
    # Claude Code Ecosystem
    "claudebox"
    "catnip"
    "claude-code-router"
    # ACP Ecosystem
    "claude-code-acp"
    "codex-acp"
    # Utilities
    "sidecar"
    "sandbox-runtime"
  ];

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
