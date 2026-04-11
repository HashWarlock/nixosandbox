# nix/mkAgentSandbox.nix
#
# Catalog-aware sandbox composition layer.
# Resolves package names from the unified catalog and delegates to mkSandboxRootfs.
#
# Usage:
#   mkAgentSandbox = import ./mkAgentSandbox.nix { inherit catalog mkSandboxRootfs; };
#   mkAgentSandbox { name = "review"; packages = [ "claude-code" "git" "ripgrep" ]; }
{ catalog, mkSandboxRootfs }:

{ name
, packages ? []
, extraPackages ? []
, env ? {}
}:

let
  resolvePackage = pname:
    if catalog.agents ? ${pname} then catalog.agents.${pname}
    else if catalog.tools ? ${pname} then catalog.tools.${pname}
    else throw "nixosandbox: unknown package '${pname}' -- not found in agents or tools catalog. Run 'nixosandbox catalog' to see available packages.";

  resolvedPackages = map resolvePackage packages;
  allPackages = resolvedPackages ++ extraPackages;
in
  mkSandboxRootfs {
    inherit name env;
    packages = allPackages;
  }
