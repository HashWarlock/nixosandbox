import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

const isLinux = process.platform === "linux";

describe("Protocol Test 8: Network Observation (Linux only)", () => {
  it.skipIf(!isLinux)(
    "observes outbound connections during execution",
    async () => {
      const rt = spawnRuntime();

      rt.send(
        makePlan({
          command: [
            "python3",
            "-c",
            "import urllib.request; urllib.request.urlopen('http://example.com')",
          ],
          manifest: {
            mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
            env: {
              HOME: "/home/sandbox",
              PATH: "/usr/bin:/bin:/usr/local/bin",
            },
            cwd: "/tmp",
          },
          policy: {
            namespaces: ["user", "pid"],
            network: { mode: "full" },
            allowedWritableTargets: ["/workspace", "/tmp"],
            strictWritePolicy: false,
          },
        }),
      );

      const events = await rt.readAllEvents();

      const validation = events[0];
      expect(validation.type).toBe("validation");
      expect((validation.payload as any).ok).toBe(true);

      const networkEvents = events.filter((e) => e.type === "network");
      expect(networkEvents.length).toBeGreaterThanOrEqual(1);

      const firstNet = networkEvents[0];
      expect((firstNet.payload as any).direction).toBe("outbound");
      expect((firstNet.payload as any).port).toBeGreaterThan(0);
      expect(typeof (firstNet.payload as any).host).toBe("string");

      const result = events[events.length - 1];
      expect(result.type).toBe("result");
      const resultPayload = result.payload as any;
      expect(resultPayload.observedConnections.length).toBeGreaterThanOrEqual(
        1,
      );

      expect(resultPayload.exitCode).toBe(0);

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );

  it.skipIf(isLinux)(
    "returns empty observations on non-Linux (no-op observer)",
    async () => {
      const rt = spawnRuntime();

      rt.send(
        makePlan({
          command: ["echo", "no-network-needed"],
          manifest: {
            mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
            env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
            cwd: "/tmp",
          },
        }),
      );

      const events = await rt.readAllEvents();

      const networkEvents = events.filter((e) => e.type === "network");
      expect(networkEvents.length).toBe(0);

      const result = events[events.length - 1];
      expect(result.type).toBe("result");
      const resultPayload = result.payload as any;
      expect(resultPayload.observedConnections).toEqual([]);

      const exit = await rt.waitForExit();
      expect(exit.code).toBe(0);
    },
  );
});
