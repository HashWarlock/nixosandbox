# nix/mkSandboxRootfs.nix
#
# Builds a minimal rootfs directory tree from a list of Nix packages.
# The output is suitable for bwrap --ro-bind <rootfs> /.
#
# Usage: mkSandboxRootfs { name = "my-env"; packages = [ pkgs.nodejs pkgs.git ]; }
{ pkgs }:

{ name, packages, env ? {} }:

let
  # Create a merged environment with all requested packages
  mergedEnv = pkgs.buildEnv {
    name = "sandbox-env-${name}";
    paths = packages;
    pathsToLink = [ "/bin" "/lib" "/lib64" "/share" "/etc" "/include" ];
    extraOutputsToInstall = [ "out" ];
  };
in
pkgs.runCommand "sandbox-${name}" {
  passthru = { inherit name env; };
} ''
  mkdir -p $out/{bin,lib,lib64,etc,usr/bin,tmp,dev,proc,workspace,home/sandbox,cache,nix/store}

  # Symlink all binaries from the merged environment
  if [ -d "${mergedEnv}/bin" ]; then
    for f in ${mergedEnv}/bin/*; do
      ln -sf "$f" "$out/bin/$(basename $f)"
    done
  fi

  # Symlink libraries
  if [ -d "${mergedEnv}/lib" ]; then
    for f in ${mergedEnv}/lib/*; do
      ln -sf "$f" "$out/lib/$(basename $f)"
    done
  fi
  if [ -d "${mergedEnv}/lib64" ]; then
    for f in ${mergedEnv}/lib64/*; do
      ln -sf "$f" "$out/lib64/$(basename $f)"
    done
  fi

  # Symlink share (man pages, etc.)
  if [ -d "${mergedEnv}/share" ]; then
    ln -sf "${mergedEnv}/share" "$out/share"
  fi

  # /usr/bin/env -- needed for #!/usr/bin/env shebangs
  ln -sf "${mergedEnv}/bin/env" "$out/usr/bin/env" 2>/dev/null || \
    ln -sf "${pkgs.coreutils}/bin/env" "$out/usr/bin/env"

  # /etc/ssl/certs -- CA certificates
  mkdir -p $out/etc/ssl/certs
  if [ -e "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ]; then
    ln -sf "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" "$out/etc/ssl/certs/ca-certificates.crt"
    ln -sf "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" "$out/etc/ssl/certs/ca-bundle.crt"
  fi

  # /etc/passwd and /etc/group -- minimal entries for sandbox user
  cat > $out/etc/passwd <<'PASSWD'
root:x:0:0:root:/root:/bin/bash
sandbox:x:1000:1000:sandbox:/home/sandbox:/bin/bash
nobody:x:65534:65534:nobody:/nonexistent:/usr/bin/nologin
PASSWD

  cat > $out/etc/group <<'GROUP'
root:x:0:
sandbox:x:1000:
nobody:x:65534:
GROUP

  # /etc/nsswitch.conf
  cat > $out/etc/nsswitch.conf <<'NSS'
passwd: files
group: files
hosts: files dns
NSS

  # /etc/hosts -- minimal
  cat > $out/etc/hosts <<'HOSTS'
127.0.0.1 localhost
::1       localhost
HOSTS

  # /etc/resolv.conf -- default DNS resolver (overridden by bwrap bind-mount when available)
  cat > $out/etc/resolv.conf <<'RESOLV'
nameserver 8.8.8.8
nameserver 8.8.4.4
RESOLV

  # Nix store reference -- keep a file that references the merged env
  # so nix-collect-garbage knows this rootfs depends on those packages
  echo "${mergedEnv}" > $out/.nix-env-reference
''
