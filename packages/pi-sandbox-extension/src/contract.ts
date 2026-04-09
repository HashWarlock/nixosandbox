/**
 * Pi Sandbox NDJSON Protocol Contract
 *
 * FROZEN — changes require a protocol version bump.
 * Protocol version: 1
 *
 * This file defines the complete set of TypeBox schemas and TypeScript
 * interfaces for all messages exchanged over stdin/stdout between the
 * TypeScript host (pi-sandbox-extension) and the Rust sandbox supervisor.
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
// Shared types
// ---------------------------------------------------------------------------

export const MountSchema = Type.Object({
  type: Type.Union([
    Type.Literal("directory"),
    Type.Literal("file"),
    Type.Literal("tmpfs"),
  ]),
  source: Type.Optional(Type.String()),
  target: Type.String(),
  writable: Type.Boolean(),
});
export type Mount = Static<typeof MountSchema>;

export const NetworkModeSchema = Type.Union([
  Type.Literal("off"),
  Type.Literal("full"),
  Type.Literal("allowlist"),
]);
export type NetworkMode = Static<typeof NetworkModeSchema>;

export const NetworkConfigSchema = Type.Object({
  mode: NetworkModeSchema,
  allowlist: Type.Optional(Type.Array(Type.String())),
});
export type NetworkConfig = Static<typeof NetworkConfigSchema>;

export const ResourceLimitsSchema = Type.Object({
  maxCpuSeconds: Type.Optional(Type.Number()),
  maxMemoryBytes: Type.Optional(Type.Number()),
  maxPids: Type.Optional(Type.Number()),
  maxOutputBytes: Type.Optional(Type.Number()),
});
export type ResourceLimits = Static<typeof ResourceLimitsSchema>;

export const ManifestSchema = Type.Object({
  mounts: Type.Array(MountSchema),
  env: Type.Record(Type.String(), Type.String()),
  cwd: Type.String(),
});
export type Manifest = Static<typeof ManifestSchema>;

export const PolicySchema = Type.Object({
  namespaces: Type.Array(Type.String()),
  network: NetworkConfigSchema,
  resourceLimits: Type.Optional(ResourceLimitsSchema),
  allowedWritableTargets: Type.Array(Type.String()),
  strictWritePolicy: Type.Boolean(),
  envAllowlist: Type.Optional(Type.Array(Type.String())),
  denyCommands: Type.Optional(Type.Array(Type.String())),
});
export type Policy = Static<typeof PolicySchema>;

// ---------------------------------------------------------------------------
// TS -> Rust messages (Inbound to the supervisor)
// ---------------------------------------------------------------------------

export const PlanPayloadSchema = Type.Object({
  version: Type.Number(),
  sessionId: Type.String(),
  executionId: Type.String(),
  requestedProfile: Type.String(),
  runtimeBaseName: Type.Optional(Type.String()),
  manifest: ManifestSchema,
  policy: PolicySchema,
  command: Type.Array(Type.String()),
});
export type PlanPayload = Static<typeof PlanPayloadSchema>;

export interface PlanMessage {
  type: "plan";
  payload: PlanPayload;
}

export const CancelPayloadSchema = Type.Object({
  reason: Type.Optional(Type.String()),
});
export type CancelPayload = Static<typeof CancelPayloadSchema>;

export interface CancelMessage {
  type: "cancel";
  payload: CancelPayload;
}

export type InboundMessage = PlanMessage | CancelMessage;

// ---------------------------------------------------------------------------
// Rust -> TS messages (Outbound from the supervisor)
// ---------------------------------------------------------------------------

export const EffectiveNetworkSchema = Type.Object({
  requested: NetworkModeSchema,
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
