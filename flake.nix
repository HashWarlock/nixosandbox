{
  description = "nixosandbox -- reproducible, isolated sandbox environments";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
  };

  outputs = { self, nixpkgs }:
  let
    # Sandbox rootfs is always Linux
    linuxSystem = "x86_64-linux";
    linuxPkgs = nixpkgs.legacyPackages.${linuxSystem};
    mkSandboxRootfs = import ./nix/mkSandboxRootfs.nix { pkgs = linuxPkgs; };

    # Helper: load a profile JSON and resolve package names to nixpkgs attrs
    loadProfile = path:
      let
        spec = builtins.fromJSON (builtins.readFile path);
        resolvedPkgs = map (name:
          if builtins.hasAttr name linuxPkgs
          then builtins.getAttr name linuxPkgs
          else throw "nixosandbox: unknown package '${name}' in profile ${spec.name}"
        ) spec.packages;
      in
        mkSandboxRootfs {
          name = spec.name;
          packages = resolvedPkgs;
          env = spec.env or {};
        };

    # Rootfs derivations (always x86_64-linux, buildable from any host)
    sandboxPackages = {
      sandbox-build-install = loadProfile ./nix/profiles/build-install.json;
      sandbox-offline-review = loadProfile ./nix/profiles/offline-review.json;
      sandbox-strict = loadProfile ./nix/profiles/strict.json;
      sandbox-debug-network = loadProfile ./nix/profiles/debug-network.json;
    };

    # All systems that can build/use nixosandbox
    supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];

    forAllSystems = f: nixpkgs.lib.genAttrs supportedSystems f;
  in
  {
    # Library function for custom rootfs
    lib.mkSandboxRootfs = mkSandboxRootfs;

    packages = forAllSystems (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
        sandboxPackages // {
          nixosandbox = pkgs.rustPlatform.buildRustPackage {
            pname = "nixosandbox";
            version = "0.1.0";
            src = ./crates/nixosandbox;
            cargoLock.lockFile = ./crates/nixosandbox/Cargo.lock;
          };

          default = self.packages.${system}.nixosandbox;
        }
    );

    devShells = forAllSystems (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        default = pkgs.mkShell {
          name = "nixosandbox-dev";
          buildInputs = with pkgs; [
            rustc
            cargo
            pkg-config
            jq
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            bubblewrap
          ];
        };
      }
    );
  };
}
