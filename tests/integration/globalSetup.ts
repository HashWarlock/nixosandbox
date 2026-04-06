import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const CRATE_DIR = resolve(import.meta.dirname, "../../crates/pi-sandbox-runtime");

export async function setup() {
  console.log("Building pi-sandbox-runtime for integration tests...");
  execFileSync("cargo", ["build", "--release"], {
    cwd: CRATE_DIR,
    stdio: "inherit",
  });

  const binaryPath = resolve(CRATE_DIR, "target/release/pi-sandbox-runtime");
  if (!existsSync(binaryPath)) {
    throw new Error(`Binary not found at ${binaryPath}`);
  }

  process.env.RUNTIME_BINARY_PATH = binaryPath;
  console.log(`Runtime binary: ${binaryPath}`);
}
