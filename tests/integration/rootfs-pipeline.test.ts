/**
 * Linux native integration tests for the rootfs pipeline.
 *
 * Requires: Nix + bwrap on Linux.
 * Gate: RUN_INTEGRATION_TESTS=1
 *
 * Run: RUN_INTEGRATION_TESTS=1 npx vitest run rootfs-pipeline.test.ts
 */
import { describe, it, expect, afterAll } from "vitest";
import { build, create, execCmd, list, destroy } from "./helpers.js";

const RUN = process.env.RUN_INTEGRATION_TESTS === "1";

describe.skipIf(!RUN)("Rootfs Pipeline (Linux native)", () => {
  const sessionsToCleanup: string[] = [];

  afterAll(() => {
    for (const id of sessionsToCleanup) {
      try {
        destroy(id);
      } catch {
        // Best-effort cleanup
      }
    }
  });

  it("build strict profile returns a valid Nix store path", () => {
    const result = build(["--profile", "strict", "--json"]);
    expect(result.exitCode).toBe(0);

    const parsed = JSON.parse(result.stdout);
    expect(parsed.rootfsPath).toBeDefined();
    expect(parsed.rootfsPath).toMatch(/^\/nix\/store\//);
  });

  it("create session returns session ID and metadata", () => {
    const { sessionId, metadata } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    expect(sessionId).toBeDefined();
    expect(sessionId.length).toBe(8);
    expect(metadata.profile).toBe("strict");
    expect(metadata.rootfsPath).toMatch(/^\/nix\/store\//);
  });

  it("exec echo prints hello and exits 0", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "hello"]);
    expect(result.exitCode).toBe(0);

    const stdoutEvents = result.events.filter((e) => e.type === "stdout");
    const helloEvent = stdoutEvents.find((e) =>
      ((e.payload as any).data as string).includes("hello"),
    );
    expect(helloEvent).toBeDefined();
  });

  it("exec verifies rootfs directory structure", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["ls", "/"]);
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    // Rootfs should have sandbox dirs
    expect(stdout).toContain("bin");
    expect(stdout).toContain("etc");
    expect(stdout).toContain("workspace");
  });

  it("exec verifies sandbox user exists", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["cat", "/etc/passwd"]);
    expect(result.exitCode).toBe(0);

    const stdout = result.events
      .filter((e) => e.type === "stdout")
      .map((e) => (e.payload as any).data as string)
      .join("\n");

    expect(stdout).toContain("sandbox");
  });

  it("exec json mode produces lifecycle + stdout + result events", async () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const result = await execCmd(sessionId, ["echo", "test"]);
    expect(result.exitCode).toBe(0);

    // Must have: lifecycle(started), stdout(test), lifecycle(exited), result
    const started = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "started",
    );
    expect(started).toBeDefined();

    const stdout = result.events.find(
      (e) =>
        e.type === "stdout" &&
        ((e.payload as any).data as string).includes("test"),
    );
    expect(stdout).toBeDefined();

    const exited = result.events.find(
      (e) =>
        e.type === "lifecycle" && (e.payload as any).event === "exited",
    );
    expect(exited).toBeDefined();

    const resultEvent = result.events.find((e) => e.type === "result") as any;
    expect(resultEvent).toBeDefined();
    expect(resultEvent.payload.exitCode).toBe(0);
    expect(resultEvent.payload.timedOut).toBe(false);
    expect(resultEvent.payload.durationMs).toBeGreaterThan(0);

    // Sequence numbers strictly increasing
    const sequenced = result.events.filter(
      (e) => (e as any).sequence !== undefined,
    );
    for (let i = 1; i < sequenced.length; i++) {
      expect((sequenced[i] as any).sequence).toBeGreaterThan(
        (sequenced[i - 1] as any).sequence,
      );
    }
  });

  it("list sessions shows the created session", () => {
    const { sessionId } = create(["--profile", "strict"]);
    sessionsToCleanup.push(sessionId);

    const { sessions } = list();
    const found = sessions.find(
      (s) => (s as any).sessionId === sessionId,
    );
    expect(found).toBeDefined();
  });

  it("destroy session removes it from list", () => {
    const { sessionId } = create(["--profile", "strict"]);

    const exitCode = destroy(sessionId);
    expect(exitCode).toBe(0);

    const { sessions } = list();
    const found = sessions.find(
      (s) => (s as any).sessionId === sessionId,
    );
    expect(found).toBeUndefined();
  });
});
