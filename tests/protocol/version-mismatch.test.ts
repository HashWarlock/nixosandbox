import { describe, expect, it } from "vitest";
import { makePlan, spawnRuntime } from "./helpers.js";

describe("Protocol Test 1: Version Mismatch", () => {
  it("rejects unsupported protocol version with VERSION_MISMATCH", async () => {
    const rt = spawnRuntime();
    rt.send(makePlan({ version: 99 }));

    const validation = await rt.readline();
    expect(validation.type).toBe("validation");
    expect((validation.payload as any).ok).toBe(false);
    expect((validation.payload as any).errors).toHaveLength(1);
    expect((validation.payload as any).errors[0].code).toBe("VERSION_MISMATCH");
    expect((validation.payload as any).effectiveState).toBeNull();

    const exit = await rt.waitForExit();
    expect(exit.code).toBe(0);
  });
});
