/**
 * Runtime Base
 *
 * Resolves host binary/library paths into read-only mount lists that the
 * sandbox supervisor can consume.  A "base" is created once per host and
 * fingerprinted so that stale sessions can be detected.
 */

import { createHash } from "node:crypto";
import { statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import type { Mount } from "./contract.js";

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

export interface RuntimeBase {
  name: string;
  fingerprint: string;
  resolveBundleMounts(bundles: string[]): Mount[];
}

// ---------------------------------------------------------------------------
// Bundle specification
// ---------------------------------------------------------------------------

type BundleSpec =
  | { kind: "directory"; path: string }
  | { kind: "file"; path: string }
  | { kind: "binary"; binary: string };

const BUNDLE_SPECS: Record<string, BundleSpec[]> = {
  core: [
    { kind: "directory", path: "/usr/bin" },
    { kind: "directory", path: "/usr/lib" },
    { kind: "directory", path: "/lib" },
    { kind: "directory", path: "/lib64" },
  ],
  certs: [{ kind: "file", path: "/etc/ssl/certs/ca-certificates.crt" }],
  git: [{ kind: "binary", binary: "git" }],
  node: [{ kind: "binary", binary: "node" }],
  python: [{ kind: "binary", binary: "python3" }],
  rust: [{ kind: "binary", binary: "cargo" }],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Resolve the full path of a binary using `which`.
 * Uses execFileSync to avoid shell injection.
 * Returns null if the binary is not found on the host.
 */
function resolveBinary(binaryName: string): string | null {
  try {
    const result = execFileSync("which", [binaryName], {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    return result.trim() || null;
  } catch {
    return null;
  }
}

function specToMount(spec: BundleSpec): Mount | null {
  if (spec.kind === "directory") {
    // Directories may not exist on all hosts (e.g. /lib64 on macOS)
    try {
      statSync(spec.path);
    } catch {
      return null;
    }
    return {
      type: "directory",
      source: spec.path,
      target: spec.path,
      writable: false,
    };
  }

  if (spec.kind === "file") {
    try {
      statSync(spec.path);
    } catch {
      return null;
    }
    return {
      type: "file",
      source: spec.path,
      target: spec.path,
      writable: false,
    };
  }

  // Binary: resolve via `which`
  const resolved = resolveBinary(spec.binary);
  if (!resolved) return null;
  return {
    type: "file",
    source: resolved,
    target: resolved,
    writable: false,
  };
}

/**
 * Compute a fingerprint over the set of resolved paths + their mtimes.
 * Sorting ensures the fingerprint is order-independent.
 */
function computeFingerprint(mounts: Mount[]): string {
  const entries: string[] = [];
  for (const mount of mounts) {
    const src = mount.source ?? mount.target;
    let mtime = 0;
    try {
      mtime = statSync(src).mtimeMs;
    } catch {
      // If stat fails, use 0 — the path will simply not exist at runtime
    }
    entries.push(`${src}:${mtime}`);
  }
  entries.sort();
  return createHash("sha256").update(entries.join("\n")).digest("hex");
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createHostDerivedBase(): RuntimeBase {
  // Resolve all known bundle specs upfront so fingerprint computation is fast
  // when callers request specific bundle subsets.
  const allMountsByBundle: Map<string, Mount[]> = new Map();

  for (const [bundleName, specs] of Object.entries(BUNDLE_SPECS)) {
    const mounts: Mount[] = [];
    for (const spec of specs) {
      const mount = specToMount(spec);
      if (mount !== null) {
        mounts.push(mount);
      }
    }
    allMountsByBundle.set(bundleName, mounts);
  }

  // Build fingerprint from ALL resolved mounts
  const allMounts: Mount[] = [];
  for (const mounts of allMountsByBundle.values()) {
    allMounts.push(...mounts);
  }
  const fingerprint = computeFingerprint(allMounts);

  return {
    name: "host-derived",
    fingerprint,

    resolveBundleMounts(bundles: string[]): Mount[] {
      const seen = new Set<string>();
      const result: Mount[] = [];

      for (const bundleName of bundles) {
        const mounts = allMountsByBundle.get(bundleName);
        if (!mounts) continue;
        for (const mount of mounts) {
          const key = `${mount.source ?? mount.target}:${mount.target}`;
          if (!seen.has(key)) {
            seen.add(key);
            result.push(mount);
          }
        }
      }

      return result;
    },
  };
}
