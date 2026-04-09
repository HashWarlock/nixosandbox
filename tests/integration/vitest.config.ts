import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["*.test.ts"],
    globalSetup: "./globalSetup.ts",
    testTimeout: 120000, // 2 minutes — Nix builds can be slow
  },
});
