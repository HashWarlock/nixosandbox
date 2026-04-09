use std::process::{Command, Stdio};
use std::path::Path;

use crate::spec::SandboxSpec;

/// Find the flake root by looking for flake.nix.
pub fn find_flake_root() -> Result<String, String> {
    if let Ok(root) = std::env::var("NIXOSANDBOX_FLAKE_ROOT") {
        if Path::new(&root).join("flake.nix").exists() {
            return Ok(root);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        while let Some(d) = dir {
            if d.join("flake.nix").exists() {
                return Ok(d.to_string_lossy().to_string());
            }
            dir = d.parent().map(|p| p.to_path_buf());
        }
    }
    if Path::new("flake.nix").exists() {
        return Ok(std::env::current_dir().map_err(|e| format!("cwd: {e}"))?.to_string_lossy().to_string());
    }
    Err("Could not find flake.nix. Set NIXOSANDBOX_FLAKE_ROOT or run from repo root.".to_string())
}

/// Build a rootfs for a built-in profile. Returns the Nix store path.
pub fn build_profile(profile_name: &str) -> Result<String, String> {
    let flake_root = find_flake_root()?;
    nix_build(&format!("{}#sandbox-{}", flake_root, profile_name))
}

/// Build a rootfs from a custom spec. Returns the Nix store path.
pub fn build_spec(spec: &SandboxSpec) -> Result<String, String> {
    let flake_root = find_flake_root()?;
    let packages_nix = spec.packages.iter().map(|p| format!("pkgs.{}", p)).collect::<Vec<_>>().join(" ");
    let env_nix = spec.env.iter().map(|(k, v)| format!("\"{}\" = \"{}\";", k, v)).collect::<Vec<_>>().join(" ");
    let expr = format!(
        r#"let pkgs = import (builtins.getFlake "{}").inputs.nixpkgs {{}}; mkSandboxRootfs = import {}/nix/mkSandboxRootfs.nix {{ inherit pkgs; }}; in mkSandboxRootfs {{ name = "{}"; packages = [ {} ]; env = {{ {} }}; }}"#,
        flake_root, flake_root, spec.name, packages_nix, env_nix
    );
    nix_build_expr(&expr)
}

fn nix_build(flake_attr: &str) -> Result<String, String> {
    let output = Command::new("nix")
        .args(["build", flake_attr, "--no-link", "--print-out-paths"])
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .output().map_err(|e| format!("nix build: {e}"))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { Err("nix build produced no output".into()) } else { Ok(path) }
    } else {
        Err(format!("nix build failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

fn nix_build_expr(expr: &str) -> Result<String, String> {
    let output = Command::new("nix")
        .args(["build", "--impure", "--expr", expr, "--no-link", "--print-out-paths"])
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .output().map_err(|e| format!("nix build --expr: {e}"))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() { Err("nix build --expr produced no output".into()) } else { Ok(path) }
    } else {
        Err(format!("nix build --expr failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

/// Check if a rootfs path looks valid.
pub fn validate_rootfs(rootfs_path: &str) -> Result<(), String> {
    let root = Path::new(rootfs_path);
    if !root.exists() { return Err(format!("rootfs not found: {rootfs_path}")); }
    if !root.join("bin").exists() { return Err(format!("rootfs missing /bin: {rootfs_path}")); }
    if !root.join("etc").exists() { return Err(format!("rootfs missing /etc: {rootfs_path}")); }
    Ok(())
}
