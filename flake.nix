{
  description = "nixosandbox -- reproducible, isolated sandbox environments";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
  };

  outputs = { self, nixpkgs }:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    mkSandboxRootfs = import ./nix/mkSandboxRootfs.nix { inherit pkgs; };

    # Helper: load a profile JSON and resolve package names to nixpkgs attrs
    loadProfile = path:
      let
        spec = builtins.fromJSON (builtins.readFile path);
        resolvedPkgs = map (name:
          if builtins.hasAttr name pkgs
          then builtins.getAttr name pkgs
          else throw "nixosandbox: unknown package '${name}' in profile ${spec.name}"
        ) spec.packages;
      in
        mkSandboxRootfs {
          name = spec.name;
          packages = resolvedPkgs;
          env = spec.env or {};
        };
  in
  {
    # Library function for custom rootfs
    lib.mkSandboxRootfs = mkSandboxRootfs;

    packages.${system} = {
      # Pre-built rootfs for each profile
      sandbox-build-install = loadProfile ./nix/profiles/build-install.json;
      sandbox-offline-review = loadProfile ./nix/profiles/offline-review.json;
      sandbox-strict = loadProfile ./nix/profiles/strict.json;
      sandbox-debug-network = loadProfile ./nix/profiles/debug-network.json;

      nixosandbox = pkgs.rustPlatform.buildRustPackage {
        pname = "nixosandbox";
        version = "0.1.0";
        src = ./crates/nixosandbox;
        cargoLock.lockFile = ./crates/nixosandbox/Cargo.lock;
      };

      default = self.packages.${system}.nixosandbox;
    };

    devShells.${system}.default = pkgs.mkShell {
      name = "nixosandbox-dev";
      buildInputs = with pkgs; [
        rustc
        cargo
        pkg-config
        bubblewrap
        jq
      ];
    };
  };
}
