/**
 * Sandbox Profiles
 *
 * Hardcoded execution profiles that bundle together network, bundle,
 * resource-limit, and security policy settings.
 */

export interface Profile {
  name: string;
  description: string;
  network: { mode: "off" | "full" | "allowlist" };
  bundles: string[];
  resourceLimits?: {
    maxCpuSeconds?: number;
    maxMemoryBytes?: number;
    maxPids?: number;
    maxOutputBytes?: number;
  };
  allowedWritableTargets: string[];
  strictWritePolicy: boolean;
  namespaces: string[];
  envAllowlist: string[];
  denyCommands: string[];
}

const PROFILES: Profile[] = [
  {
    name: "offline-review",
    description:
      "Offline code review environment: no network, core tools plus git.",
    network: { mode: "off" },
    bundles: ["core", "certs", "git"],
    allowedWritableTargets: ["/workspace", "/tmp"],
    strictWritePolicy: true,
    namespaces: ["pid", "mount", "net", "uts", "ipc"],
    envAllowlist: ["PATH", "HOME", "LANG", "TERM"],
    denyCommands: [],
  },
  {
    name: "strict",
    description:
      "Strict offline sandbox: no network, minimal toolset, tight write policy.",
    network: { mode: "off" },
    bundles: ["core", "certs"],
    allowedWritableTargets: ["/workspace", "/tmp"],
    strictWritePolicy: true,
    namespaces: ["pid", "mount", "net", "uts", "ipc"],
    envAllowlist: ["PATH", "HOME", "LANG"],
    denyCommands: [],
  },
  {
    name: "build-install",
    description:
      "Full-network build environment with package managers and compilers.",
    network: { mode: "full" },
    bundles: ["core", "certs", "git", "node", "python", "rust"],
    resourceLimits: {
      maxCpuSeconds: 600,
      maxMemoryBytes: 2 * 1024 * 1024 * 1024, // 2 GiB
      maxPids: 512,
    },
    allowedWritableTargets: ["/workspace", "/home/sandbox", "/cache", "/tmp"],
    strictWritePolicy: false,
    namespaces: ["pid", "mount", "uts", "ipc"],
    envAllowlist: [
      "PATH",
      "HOME",
      "LANG",
      "TERM",
      "NODE_ENV",
      "npm_config_cache",
      "CARGO_HOME",
      "RUSTUP_HOME",
      "PYTHONPATH",
    ],
    denyCommands: [],
  },
  {
    name: "debug-network",
    description:
      "Full-network debug environment with node and python, for diagnostics.",
    network: { mode: "full" },
    bundles: ["core", "certs", "git", "node", "python"],
    allowedWritableTargets: ["/workspace", "/home/sandbox", "/cache", "/tmp"],
    strictWritePolicy: false,
    namespaces: ["pid", "mount", "uts", "ipc"],
    envAllowlist: [
      "PATH",
      "HOME",
      "LANG",
      "TERM",
      "NODE_ENV",
      "npm_config_cache",
      "PYTHONPATH",
    ],
    denyCommands: [],
  },
];

/** The profile used when the caller does not specify one. */
export const DEFAULT_PROFILE = "build-install";

const PROFILE_MAP: Map<string, Profile> = new Map(
  PROFILES.map((p) => [p.name, p]),
);

/** Returns the named profile, or throws if unknown. */
export function getProfile(name: string): Profile {
  const profile = PROFILE_MAP.get(name);
  if (!profile) {
    throw new Error(
      `Unknown profile: "${name}". Available: ${PROFILES.map((p) => p.name).join(", ")}`,
    );
  }
  return profile;
}

/** Returns a list of all profile names. */
export function listProfiles(): string[] {
  return PROFILES.map((p) => p.name);
}
