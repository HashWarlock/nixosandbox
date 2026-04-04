/**
 * Runtime Client
 *
 * Subprocess client that spawns the Rust sandbox supervisor binary,
 * communicates over NDJSON stdio, and synthesizes crash results on
 * abnormal exit.
 */

import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import type {
  PlanPayload,
  ResultPayload,
  StreamEvent,
  ValidationMessage,
  ValidationPayload,
} from "./contract.js";
import { synthesizeCrashResult } from "./crash-synthesis.js";

export interface RuntimeClientOptions {
  binaryPath: string;
  timeout?: number;
}

export interface ExecutionHandle {
  validation: Promise<ValidationMessage>;
  result: Promise<ResultPayload>;
  cancel(reason?: string): void;
  readonly stderr: string;
}

type ClientState =
  | "spawned"
  | "plan_sent"
  | "validation_received"
  | "streaming"
  | "result_received"
  | "crashed";

export class RuntimeClient {
  private readonly options: RuntimeClientOptions;

  constructor(options: RuntimeClientOptions) {
    this.options = options;
  }

  execute(
    plan: PlanPayload,
    onEvent?: (event: StreamEvent) => void,
  ): ExecutionHandle {
    const { binaryPath, timeout } = this.options;
    const startMs = Date.now();

    let state: ClientState = "spawned";
    let stderrOutput = "";
    let lastValidation: ValidationPayload | null = null;
    let timeoutHandle: ReturnType<typeof setTimeout> | null = null;
    let killHandle: ReturnType<typeof setTimeout> | null = null;

    // Promises with external resolvers
    let resolveValidation!: (msg: ValidationMessage) => void;
    let rejectValidation!: (err: Error) => void;
    let resolveResult!: (payload: ResultPayload) => void;
    // result promise never rejects — crashes are synthesized into a result

    const validationPromise = new Promise<ValidationMessage>((res, rej) => {
      resolveValidation = res;
      rejectValidation = rej;
    });

    const resultPromise = new Promise<ResultPayload>((res) => {
      resolveResult = res;
    });

    // Spawn child process
    const child = spawn(binaryPath, [], {
      stdio: ["pipe", "pipe", "pipe"],
    });

    // Collect stderr
    child.stderr.on("data", (chunk: Buffer) => {
      stderrOutput += chunk.toString();
    });

    // Set up timeout if requested
    if (timeout != null && timeout > 0) {
      timeoutHandle = setTimeout(() => {
        timeoutHandle = null;
        child.kill("SIGTERM");
        // Follow up with SIGKILL after 5 s if the process hasn't exited
        killHandle = setTimeout(() => {
          killHandle = null;
          child.kill("SIGKILL");
        }, 5000);
      }, timeout);
    }

    const clearTimers = (): void => {
      if (timeoutHandle !== null) {
        clearTimeout(timeoutHandle);
        timeoutHandle = null;
      }
      if (killHandle !== null) {
        clearTimeout(killHandle);
        killHandle = null;
      }
    };

    // Write plan to stdin as NDJSON
    const planMessage = JSON.stringify({ type: "plan", payload: plan });
    child.stdin.write(planMessage + "\n", () => {
      state = "plan_sent";
    });
    // Keep stdin open for cancel

    // Read stdout line-by-line (NDJSON)
    const rl = createInterface({ input: child.stdout, crlfDelay: Infinity });

    rl.on("line", (line: string) => {
      const trimmed = line.trim();
      if (!trimmed) return;

      let msg: unknown;
      try {
        msg = JSON.parse(trimmed);
      } catch {
        // Malformed line — ignore
        return;
      }

      if (
        typeof msg !== "object" ||
        msg === null ||
        !("type" in msg)
      ) {
        return;
      }

      const typed = msg as { type: string; [key: string]: unknown };

      if (typed.type === "validation") {
        const vmsg = typed as unknown as ValidationMessage;
        lastValidation = vmsg.payload;
        state = vmsg.payload.ok ? "streaming" : "validation_received";
        resolveValidation(vmsg);

        // If validation failed the Rust side will not emit a result message.
        // We wait for the process to exit and synthesize.
        return;
      }

      if (typed.type === "result") {
        clearTimers();
        state = "result_received";
        const payload = (typed as { type: "result"; payload: ResultPayload })
          .payload;
        resolveResult(payload);
        return;
      }

      // All other message types are stream events
      if (onEvent) {
        try {
          onEvent(typed as unknown as StreamEvent);
        } catch {
          // Swallow errors from consumer callbacks
        }
      }
    });

    rl.on("close", () => {
      // stdout closed — if we haven't received a result yet, synthesize one
      if (state !== "result_received") {
        // We don't have exit code/signal yet at this point; wait for the
        // close event on the child process to call resolveResult.
        // Nothing to do here — the child 'close' handler does the synthesis.
      }
    });

    child.on("close", (exitCode: number | null, signal: string | null) => {
      clearTimers();

      // Ensure validation promise is settled (in case process died before
      // emitting a validation message)
      if (state === "spawned" || state === "plan_sent") {
        const durationMs = Date.now() - startMs;
        const crashResult = synthesizeCrashResult(
          null,
          plan,
          exitCode,
          signal,
          durationMs,
        );
        // Build a synthetic validation message
        const syntheticValidation: ValidationMessage = {
          type: "validation",
          v: plan.version,
          payload: {
            ok: false,
            errors: [
              {
                code: "MISSING_REQUIRED_FIELD",
                message: "Process exited before sending validation message",
              },
            ],
            warnings: [],
            effectiveState: null,
          },
        };
        resolveValidation(syntheticValidation);
        state = "crashed";
        resolveResult(crashResult);
        return;
      }

      if (state !== "result_received") {
        const durationMs = Date.now() - startMs;
        state = "crashed";
        const crashResult = synthesizeCrashResult(
          lastValidation,
          plan,
          exitCode,
          signal,
          durationMs,
        );
        resolveResult(crashResult);
      }
    });

    child.on("error", (err: Error) => {
      clearTimers();
      // Process failed to spawn
      if (state === "spawned" || state === "plan_sent") {
        rejectValidation(err);
      }
      if (state !== "result_received") {
        const durationMs = Date.now() - startMs;
        state = "crashed";
        const crashResult = synthesizeCrashResult(
          lastValidation,
          plan,
          null,
          null,
          durationMs,
        );
        resolveResult(crashResult);
      }
    });

    const handle: ExecutionHandle = {
      validation: validationPromise,
      result: resultPromise,
      cancel(reason?: string): void {
        const cancelMessage = JSON.stringify({
          type: "cancel",
          payload: { reason },
        });
        try {
          child.stdin.write(cancelMessage + "\n");
        } catch {
          // stdin may already be closed if the process exited
        }
      },
      get stderr(): string {
        return stderrOutput;
      },
    };

    return handle;
  }
}
