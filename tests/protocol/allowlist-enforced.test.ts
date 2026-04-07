import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";
import { platform } from "node:os";

describe("Protocol Test 8: Allowlist Enforcement", () => {
  const isLinux = platform() === "linux";

  it("enforces allowlist on Linux with bwrap and iptables", async () => {
    if (!isLinux) {
      console.log("Skipping: allowlist enforcement requires Linux with bwrap");
      return;
    }

    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "allowlist-enforced"],
        manifest: {
          mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin:/usr/sbin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "net"],
          network: {
            mode: "allowlist",
            allowlist: ["localhost"],
          },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const vPayload = validation.payload as any;
    expect(vPayload.ok).toBe(true);

    const eff = vPayload.effectiveState;

    if (eff.network.enforcement === "enforced") {
      expect(eff.network.actual).toBe("allowlist");
      expect(eff.network.degraded).toBe(false);
      expect(eff.resolvedAllowlist.length).toBeGreaterThan(0);
      expect(eff.resolvedAllowlist[0].hostname).toBe("localhost");
      expect(eff.resolvedAllowlist[0].resolved).toBe(true);
    } else {
      expect(eff.network.actual).toBe("full");
      expect(eff.network.enforcement).toBe("observed");
      expect(eff.network.degraded).toBe(true);
    }

    const result = events[events.length - 1];
    expect(result.type).toBe("result");
    const rPayload = result.payload as any;
    expect(rPayload.exitCode).toBe(0);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });

  it("degrades allowlist to observed on macOS", async () => {
    if (isLinux) {
      console.log("Skipping: this test is for macOS degradation");
      return;
    }

    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "allowlist-degraded"],
        manifest: {
          mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "net"],
          network: {
            mode: "allowlist",
            allowlist: ["example.com"],
          },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const vPayload = validation.payload as any;
    expect(vPayload.ok).toBe(true);

    expect(vPayload.effectiveState.network.requested).toBe("allowlist");
    expect(vPayload.effectiveState.network.actual).toBe("full");
    expect(vPayload.effectiveState.network.enforcement).toBe("observed");
    expect(vPayload.effectiveState.network.degraded).toBe(true);

    const warnings: any[] = vPayload.warnings ?? [];
    const found = warnings.some(
      (w: any) => w.code === "ALLOWLIST_NOT_ENFORCED",
    );
    expect(found).toBe(true);

    const result = events[events.length - 1];
    expect(result.type).toBe("result");

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });

  it("emits DNS_RESOLUTION_PARTIAL for unresolvable hostname", async () => {
    const rt = spawnRuntime();

    rt.send(
      makePlan({
        command: ["echo", "dns-partial"],
        manifest: {
          mounts: [{ type: "tmpfs", target: "/tmp", writable: true }],
          env: { HOME: "/home/sandbox", PATH: "/usr/bin:/bin" },
          cwd: "/tmp",
        },
        policy: {
          namespaces: ["user", "pid", "net"],
          network: {
            mode: "allowlist",
            allowlist: ["this-host-definitely-does-not-exist.invalid"],
          },
          allowedWritableTargets: ["/workspace", "/tmp"],
          strictWritePolicy: false,
        },
      }),
    );

    const events = await rt.readAllEvents();

    const validation = events[0];
    expect(validation.type).toBe("validation");
    const vPayload = validation.payload as any;
    expect(vPayload.ok).toBe(true);

    const warnings: any[] = vPayload.warnings ?? [];
    const dnsWarning = warnings.find(
      (w: any) =>
        w.code === "DNS_RESOLUTION_PARTIAL" ||
        w.code === "ALLOWLIST_DNS_FAILED",
    );
    expect(dnsWarning).toBeDefined();

    expect(vPayload.effectiveState.network.actual).toBe("full");
    expect(vPayload.effectiveState.network.degraded).toBe(true);

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
