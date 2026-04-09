use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

/// A sandbox environment specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSpec {
    pub name: String,
    pub packages: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_network")]
    pub network: String,
    #[serde(default = "default_namespaces")]
    pub namespaces: Vec<String>,
    #[serde(default = "default_writable")]
    pub writable: Vec<String>,
}

fn default_network() -> String { "full".to_string() }

fn default_namespaces() -> Vec<String> {
    vec!["pid".to_string(), "mount".to_string(), "uts".to_string(), "ipc".to_string()]
}

fn default_writable() -> Vec<String> {
    vec!["/workspace".to_string(), "/home/sandbox".to_string(), "/cache".to_string(), "/tmp".to_string()]
}

/// Load a spec from a JSON file path.
pub fn load_spec(path: &str) -> Result<SandboxSpec, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read spec file '{}': {}", path, e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse spec file '{}': {}", path, e))
}

/// Load a built-in profile by name.
pub fn load_profile(name: &str, flake_root: &str) -> Result<SandboxSpec, String> {
    let path = format!("{}/nix/profiles/{}.json", flake_root, name);
    if !Path::new(&path).exists() {
        return Err(format!(
            "unknown profile '{}'. Available: build-install, offline-review, strict, debug-network",
            name
        ));
    }
    load_spec(&path)
}

/// Validate a spec for basic correctness.
pub fn validate_spec(spec: &SandboxSpec) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if spec.name.is_empty() {
        errors.push("spec.name must not be empty".to_string());
    }
    if spec.packages.is_empty() {
        errors.push("spec.packages must not be empty".to_string());
    }
    match spec.network.as_str() {
        "off" | "full" => {}
        other => errors.push(format!("spec.network must be 'off' or 'full', got '{}'", other)),
    }
    for ns in &spec.namespaces {
        match ns.as_str() {
            "pid" | "mount" | "uts" | "ipc" | "net" | "user" | "cgroup" => {}
            other => errors.push(format!("unknown namespace '{}' in spec.namespaces", other)),
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_spec() {
        let json = r#"{"name":"test","packages":["bash"]}"#;
        let spec: SandboxSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.name, "test");
        assert_eq!(spec.packages, vec!["bash"]);
        assert_eq!(spec.network, "full");
        assert_eq!(spec.namespaces, vec!["pid", "mount", "uts", "ipc"]);
    }

    #[test]
    fn deserialize_full_spec() {
        let json = r#"{"name":"web","packages":["nodejs_22","git"],"env":{"NODE_ENV":"dev"},"network":"off","namespaces":["pid","net"],"writable":["/tmp"]}"#;
        let spec: SandboxSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.network, "off");
        assert_eq!(spec.env.get("NODE_ENV").unwrap(), "dev");
    }

    #[test]
    fn validate_valid_spec() {
        let spec = SandboxSpec {
            name: "test".to_string(), packages: vec!["bash".to_string()],
            env: HashMap::new(), network: "full".to_string(),
            namespaces: vec!["pid".to_string()], writable: vec!["/tmp".to_string()],
        };
        assert!(validate_spec(&spec).is_ok());
    }

    #[test]
    fn validate_empty_name_fails() {
        let spec = SandboxSpec {
            name: "".to_string(), packages: vec!["bash".to_string()],
            env: HashMap::new(), network: "full".to_string(),
            namespaces: vec![], writable: vec![],
        };
        assert!(validate_spec(&spec).unwrap_err().iter().any(|e| e.contains("name")));
    }

    #[test]
    fn validate_bad_network_fails() {
        let spec = SandboxSpec {
            name: "test".to_string(), packages: vec!["bash".to_string()],
            env: HashMap::new(), network: "allowlist".to_string(),
            namespaces: vec![], writable: vec![],
        };
        assert!(validate_spec(&spec).unwrap_err().iter().any(|e| e.contains("network")));
    }
}
