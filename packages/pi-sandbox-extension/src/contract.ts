/**
 * Pi Sandbox NDJSON Protocol Contract
 *
 * FROZEN — changes require a protocol version bump.
 * Protocol version: 1
 *
 * This file defines the TypeBox schemas and TypeScript interfaces for
 * outbound messages received from `nixosandbox exec --json` NDJSON output.
 */

import { Type, type Static } from "@sinclair/typebox";

// ---------------------------------------------------------------------------
// Protocol version
// ---------------------------------------------------------------------------

export const PROTOCOL_VERSION = 1 as const;

// ---------------------------------------------------------------------------
// Error and warning codes
// ---------------------------------------------------------------------------

export type ErrorCode =
  | "VERSION_MISMATCH"
  | "RW_TARGET_NOT_ALLOWED"
  | "COMMAND_DENIED"
  | "INVALID_MOUNT"
  | "MISSING_REQUIRED_FIELD";

export type WarningCode =
  | "ALLOWLIST_NOT_ENFORCED"
  | "NAMESPACE_DEGRADED"
  | "RESOURCE_LIMIT_IGNORED"
  | "DNS_RESOLUTION_PARTIAL"
  | "ALLOWLIST_DNS_FAILED"
  | "ENFORCEMENT_LEAK"
  | "IPTABLES_NOT_FOUND"
  | "DOCKER_NOT_AVAILABLE"
  | "DOCKER_SIDECAR_RESTARTED";

// ---------------------------------------------------------------------------
// Rust -> TS messages (Outbound from the supervisor)
// ---------------------------------------------------------------------------

export const EffectiveNetworkSchema = Type.Object({
  requested: Type.Union([
    Type.Literal("off"),
    Type.Literal("full"),
    Type.Literal("allowlist"),
  ]),
  actual: Type.Union([
    Type.Literal("off"),
    Type.Literal("full"),
    Type.Literal("allowlist"),
  ]),
  enforcement: Type.Union([
    Type.Literal("enforced"),
    Type.Literal("observed"),
    Type.Literal("none"),
    Type.Literal("best_effort"),
  ]),
  degraded: Type.Boolean(),
});
export type EffectiveNetwork = Static<typeof EffectiveNetworkSchema>;

export const EffectiveStateSchema = Type.Object({
  network: EffectiveNetworkSchema,
  namespacesApplied: Type.Array(Type.String()),
  envApplied: Type.Array(Type.String()),
  resolvedAllowlist: Type.Array(
    Type.Object({
      hostname: Type.String(),
      ips: Type.Array(Type.String()),
      resolved: Type.Boolean(),
    })
  ),
  isolationBackend: Type.Union([
    Type.Literal("native"),
    Type.Literal("docker"),
    Type.Literal("none"),
  ]),
});
export type EffectiveState = Static<typeof EffectiveStateSchema>;

export interface ValidationError {
  code: ErrorCode;
  message: string;
  field?: string;
}

export interface ValidationWarning {
  code: WarningCode;
  message: string;
}

export interface ValidationPayload {
  ok: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
  effectiveState: EffectiveState | null;
}

export interface ValidationMessage {
  type: "validation";
  v: number;
  payload: ValidationPayload;
}

// ---------------------------------------------------------------------------
// Streamed events
// ---------------------------------------------------------------------------

export interface StdoutEvent {
  type: "stdout";
  sequence: number;
  ts: number;
  payload: { data: string };
}

export interface StderrEvent {
  type: "stderr";
  sequence: number;
  ts: number;
  payload: { data: string };
}

export interface LifecycleEvent {
  type: "lifecycle";
  sequence: number;
  ts: number;
  payload: {
    event: "started" | "cancel_requested" | "killing" | "exited";
  };
}

export interface NetworkEvent {
  type: "network";
  sequence: number;
  ts: number;
  payload: { [key: string]: unknown };
}

export interface WarningEvent {
  type: "warning";
  sequence: number;
  ts: number;
  payload: { code: WarningCode; message: string };
}

export type StreamEvent =
  | StdoutEvent
  | StderrEvent
  | LifecycleEvent
  | NetworkEvent
  | WarningEvent;

// ---------------------------------------------------------------------------
// Result message
// ---------------------------------------------------------------------------

export type TerminalState =
  | "clean_exit"
  | "killed_on_cancel"
  | "killed_on_timeout"
  | "supervisor_crash"
  | "partial_cleanup";

export interface ObservedConnection {
  host: string;
  port: number;
  timestamp: number;
}

export interface BlockedConnection {
  host: string;
  port: number;
}

export interface ReconciliationHints {
  terminalState: TerminalState;
  workspaceModified: boolean;
  cleanupSucceeded: boolean;
}

export interface ResultPayload {
  exitCode: number | null;
  signal: string | null;
  timedOut: boolean;
  durationMs: number;
  effectiveNetwork: EffectiveNetwork;
  observedConnections: ObservedConnection[];
  wouldHaveBlocked: BlockedConnection[];
  resourcePeaks?: { [key: string]: number };
  reconciliationHints: ReconciliationHints;
}

export interface ResultMessage {
  type: "result";
  v: number;
  payload: ResultPayload;
}

export type OutboundMessage = ValidationMessage | StreamEvent | ResultMessage;
