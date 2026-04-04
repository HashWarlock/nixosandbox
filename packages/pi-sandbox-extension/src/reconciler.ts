/**
 * Reconciler
 *
 * Scans all sessions on startup and recovers or tombstones orphaned sessions.
 *
 * Rules:
 *   - "active" sessions whose PID is still alive → kill PID, mark "recovered", clean tmp
 *   - "active" sessions whose PID is gone       → mark "recovered", clean tmp
 *   - "recovered" sessions older than 7 days    → tombstone
 */

import type { Session, SessionRecord } from "./session-manager.js";
import { SessionManager } from "./session-manager.js";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type RecoveryActionKind =
  | "kill_and_recover"
  | "recover"
  | "tombstone"
  | "noop";

export interface RecoveryAction {
  sessionId: string;
  action: RecoveryActionKind;
  reason: string;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Returns true if a process with the given PID is currently running.
 * Uses process.kill(pid, 0) which does not actually send a signal.
 */
function isProcessAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/**
 * Attempts to kill a process with SIGTERM.
 * Silently ignores errors (process may have already exited).
 */
function killProcess(pid: number): void {
  try {
    process.kill(pid, "SIGTERM");
  } catch {
    // Already gone — ignore
  }
}

function ageMs(isoTimestamp: string): number {
  return Date.now() - new Date(isoTimestamp).getTime();
}

// ---------------------------------------------------------------------------
// Main export
// ---------------------------------------------------------------------------

export function reconcileAll(sessionManager: SessionManager): RecoveryAction[] {
  const records: SessionRecord[] = sessionManager.list();
  const actions: RecoveryAction[] = [];

  for (const record of records) {
    const session: Session = { record, dir: sessionManager.sessionDir(record.sessionId) };

    if (record.state === "active") {
      const exec = record.activeExecution;
      let killedPid = false;

      if (exec && isProcessAlive(exec.pid)) {
        killProcess(exec.pid);
        killedPid = true;
      }

      sessionManager.updateRecord(session, { state: "recovered", activeExecution: null });
      sessionManager.cleanTmp(session);

      const action: RecoveryAction = {
        sessionId: record.sessionId,
        action: killedPid ? "kill_and_recover" : "recover",
        reason: killedPid
          ? `Killed orphaned PID ${exec!.pid} and marked session recovered`
          : "Orphaned active session (PID already gone); marked recovered",
      };
      actions.push(action);
      continue;
    }

    if (record.state === "recovered") {
      if (ageMs(record.lastActiveAt) > SEVEN_DAYS_MS) {
        const updatedSession = sessionManager.updateRecord(session, { state: "recovered" });
        sessionManager.tombstone(updatedSession);
        actions.push({
          sessionId: record.sessionId,
          action: "tombstone",
          reason: "Recovered session older than 7 days; tombstoned",
        });
      } else {
        actions.push({
          sessionId: record.sessionId,
          action: "noop",
          reason: "Recovered session within 7-day window; no action",
        });
      }
      continue;
    }

    // All other states (idle, tombstoned) — nothing to do
    actions.push({
      sessionId: record.sessionId,
      action: "noop",
      reason: `Session state "${record.state}" requires no reconciliation`,
    });
  }

  return actions;
}
