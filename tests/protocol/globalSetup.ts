import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const CRATE_DIR = resolve(import.meta.dirname, "../../crates/nixosandbox");

export async function setup() {
  console.log("Building nixosandbox...");
  execFileSync("cargo", ["build", "--release"], {
    cwd: CRATE_DIR,
    stdio: "inherit",
  });

  const binaryPath = resolve(CRATE_DIR, "target/release/nixosandbox");
  if (!existsSync(binaryPath)) {
    throw new Error(`Binary not found at ${binaryPath}`);
  }

  process.env.NIXOSANDBOX_BINARY = binaryPath;
  // Disable Docker sidecar for non-Docker tests.
  // Docker-specific tests override this via their own env.
  process.env.NIXOSANDBOX_NO_DOCKER = "1";
  console.log(`Runtime binary: ${binaryPath}`);
}
