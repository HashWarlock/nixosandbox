import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const CRATE_DIR = resolve(import.meta.dirname, "../../crates/nixosandbox");

export async function setup() {
  console.log("Building nixosandbox (release)...");
  execFileSync("cargo", ["build", "--release"], {
    cwd: CRATE_DIR,
    stdio: "inherit",
  });

  const binaryPath = resolve(CRATE_DIR, "target/release/nixosandbox");
  if (!existsSync(binaryPath)) {
    throw new Error(`Binary not found at ${binaryPath}`);
  }

  process.env.NIXOSANDBOX_BINARY = binaryPath;
  console.log(`Runtime binary: ${binaryPath}`);
}
