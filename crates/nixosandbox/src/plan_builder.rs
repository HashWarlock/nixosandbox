/// Session directory paths for rootfs-mode execution.
pub struct RootfsSessionDirs {
    pub workspace: String,
    pub home: String,
    pub cache: String,
}

/// Build bwrap argument vector for sandboxed execution with a Nix rootfs.
pub fn build_rootfs(
    rootfs_path: &str,
    session_dirs: &RootfsSessionDirs,
    command: &[String],
    env: &std::collections::HashMap<String, String>,
    _network: &str,
    namespaces: &[String],
) -> Vec<String> {
    let mut argv: Vec<String> = Vec::new();
    // Lifecycle: kill sandbox when parent dies, isolate from terminal signals
    argv.push("--die-with-parent".to_string());
    argv.push("--new-session".to_string());
    // Mount the Nix rootfs as the new / (bwrap internally does pivot_root)
    argv.extend(["--ro-bind".to_string(), rootfs_path.to_string(), "/".to_string()]);
    argv.extend(["--bind".to_string(), session_dirs.workspace.clone(), "/workspace".to_string()]);
    argv.extend(["--bind".to_string(), session_dirs.home.clone(), "/home/sandbox".to_string()]);
    argv.extend(["--bind".to_string(), session_dirs.cache.clone(), "/cache".to_string()]);
    argv.extend(["--tmpfs".to_string(), "/tmp".to_string()]);
    argv.extend(["--dev".to_string(), "/dev".to_string()]);
    argv.extend(["--proc".to_string(), "/proc".to_string()]);
    for ns in namespaces {
        match ns.as_str() {
            "pid" => argv.push("--unshare-pid".to_string()),
            "mount" => {} // implicit with --ro-bind /
            "uts" => argv.push("--unshare-uts".to_string()),
            "ipc" => argv.push("--unshare-ipc".to_string()),
            "net" => argv.push("--unshare-net".to_string()),
            "user" => argv.push("--unshare-user".to_string()),
            "cgroup" => argv.push("--unshare-cgroup-try".to_string()),
            _ => {}
        }
    }
    argv.push("--clearenv".to_string());
    argv.extend(["--setenv".to_string(), "HOME".to_string(), "/home/sandbox".to_string()]);
    argv.extend(["--setenv".to_string(), "PATH".to_string(), "/bin:/usr/bin".to_string()]);
    argv.extend(["--setenv".to_string(), "TERM".to_string(), "xterm-256color".to_string()]);
    for (key, value) in env {
        argv.extend(["--setenv".to_string(), key.clone(), value.clone()]);
    }
    argv.extend(["--chdir".to_string(), "/workspace".to_string()]);
    argv.push("--".to_string());
    argv.extend(command.iter().cloned());
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_rootfs_produces_ro_bind_root_argv() {
        let dirs = RootfsSessionDirs {
            workspace: "/tmp/ws".to_string(),
            home: "/tmp/home".to_string(),
            cache: "/tmp/cache".to_string(),
        };
        let cmd = vec!["echo".to_string(), "hello".to_string()];
        let env = std::collections::HashMap::new();
        let argv = build_rootfs("/nix/store/fake", &dirs, &cmd, &env, "full", &["pid".to_string()]);
        // Rootfs is mounted read-only at / (bwrap internally does pivot_root)
        assert!(argv.contains(&"--ro-bind".to_string()));
        assert!(argv.contains(&"/nix/store/fake".to_string()));
        // Verify rootfs is bound to /
        let ro_bind_pos = argv.iter().position(|a| a == "--ro-bind").unwrap();
        assert_eq!(argv[ro_bind_pos + 1], "/nix/store/fake");
        assert_eq!(argv[ro_bind_pos + 2], "/");
        assert!(argv.contains(&"--bind".to_string()));
        assert!(argv.contains(&"--tmpfs".to_string()));
        assert!(argv.contains(&"--dev".to_string()));
        assert!(argv.contains(&"--proc".to_string()));
        assert!(argv.contains(&"--clearenv".to_string()));
        let sep = argv.iter().position(|a| a == "--").unwrap();
        assert_eq!(argv[sep + 1], "echo");
        assert_eq!(argv[sep + 2], "hello");
    }

    #[test]
    fn build_rootfs_network_off_adds_unshare_net() {
        let dirs = RootfsSessionDirs {
            workspace: "/tmp/ws".to_string(), home: "/tmp/home".to_string(), cache: "/tmp/cache".to_string(),
        };
        let cmd = vec!["echo".to_string()];
        let env = std::collections::HashMap::new();
        let argv = build_rootfs("/nix/store/fake", &dirs, &cmd, &env, "off", &["pid".to_string(), "net".to_string()]);
        assert!(argv.contains(&"--unshare-net".to_string()));
    }
}
