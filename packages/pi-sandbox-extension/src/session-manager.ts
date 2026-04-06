/**
 * Session Manager
 *
 * Manages sandbox session directories on disk.  Each session has its own
 * subdirectory under `~/.local/share/pi-sandbox/sessions/<uuid>/`.
 */

import {
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { randomUUID } from "node:crypto";
import type { Manifest, Mount } from "./contract.js";
import type { Profile } from "./profiles.js";
import type { RuntimeBase } from "./runtime-base.js";
import type { BrowserManager } from "./browser.js";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ActiveExecution {
  executionId: string;
  pid: number;
  startedAt: string; // ISO-8601
  profileName: string;
}

export type SessionState =
  | "active"
  | "idle"
  | "recovered"
  | "tombstoned";

export interface SessionRecord {
  sessionId: string;
  state: SessionState;
  createdAt: string; // ISO-8601
  lastActiveAt: string; // ISO-8601
  runtimeBaseName: string;
  runtimeBaseFingerprint: string;
  policyHash: string;
  activeExecution: ActiveExecution | null;
  lastHeartbeat: string | null; // ISO-8601
}

export interface Session {
  record: SessionRecord;
  dir: string;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const RECORD_FILENAME = "record.json";

const SESSION_SUBDIRS = [
  "workspace",
  "artifacts",
  "logs",
  "tmp",
  "home",
  "cache",
] as const;

// ---------------------------------------------------------------------------
// SessionManager
// ---------------------------------------------------------------------------

export class SessionManager {
  private readonly baseDir: string;
  private browserManager: BrowserManager | null = null;

  setBrowserManager(bm: BrowserManager): void {
    this.browserManager = bm;
  }

  constructor(baseDir?: string) {
    this.baseDir =
      baseDir ??
      join(homedir(), ".local", "share", "pi-sandbox", "sessions");
    mkdirSync(this.baseDir, { recursive: true });
  }

  // -------------------------------------------------------------------------
  // Create
  // -------------------------------------------------------------------------

  create(runtimeBase: RuntimeBase): Session {
    const sessionId = randomUUID();
    const dir = join(this.baseDir, sessionId);

    mkdirSync(dir, { recursive: true });
    for (const sub of SESSION_SUBDIRS) {
      mkdirSync(join(dir, sub), { recursive: true });
    }

    const now = new Date().toISOString();
    const record: SessionRecord = {
      sessionId,
      state: "idle",
      createdAt: now,
      lastActiveAt: now,
      runtimeBaseName: runtimeBase.name,
      runtimeBaseFingerprint: runtimeBase.fingerprint,
      policyHash: "",
      activeExecution: null,
      lastHeartbeat: null,
    };

    this._writeRecord(dir, record);
    return { record, dir };
  }

  // -------------------------------------------------------------------------
  // Load
  // -------------------------------------------------------------------------

  load(sessionId: string): Session | null {
    const dir = join(this.baseDir, sessionId);
    const record = this._readRecord(dir);
    if (!record) return null;
    return { record, dir };
  }

  // -------------------------------------------------------------------------
  // List
  // -------------------------------------------------------------------------

  list(): SessionRecord[] {
    let entries: string[];
    try {
      entries = readdirSync(this.baseDir);
    } catch {
      return [];
    }

    const records: SessionRecord[] = [];
    for (const entry of entries) {
      const dir = join(this.baseDir, entry);
      const record = this._readRecord(dir);
      if (record) records.push(record);
    }
    return records;
  }

  // -------------------------------------------------------------------------
  // Path helpers
  // -------------------------------------------------------------------------

  getWorkspacePath(session: Session): string {
    return join(session.dir, "workspace");
  }

  getArtifactsPath(session: Session): string {
    return join(session.dir, "artifacts");
  }

  // -------------------------------------------------------------------------
  // Manifest building
  // -------------------------------------------------------------------------

  buildMountManifest(
    session: Session,
    profile: Profile,
    runtimeBase: RuntimeBase,
  ): Manifest {
    const mounts: Mount[] = [];

    // 1. Bundle mounts (read-only)
    const bundleMounts = runtimeBase.resolveBundleMounts(profile.bundles);
    mounts.push(...bundleMounts);

    // 2. Session-local directories
    const workspaceHost = this.getWorkspacePath(session);
    mounts.push({
      type: "directory",
      source: workspaceHost,
      target: "/workspace",
      writable: true,
    });

    mounts.push({
      type: "directory",
      source: join(session.dir, "home"),
      target: "/home/sandbox",
      writable: true,
    });

    mounts.push({
      type: "directory",
      source: join(session.dir, "artifacts"),
      target: "/artifacts",
      writable: true,
    });

    mounts.push({
      type: "directory",
      source: join(session.dir, "cache"),
      target: "/cache",
      writable: true,
    });

    // 3. Tmp as tmpfs
    mounts.push({
      type: "tmpfs",
      target: "/tmp",
      writable: true,
    });

    // 4. Logs dir (read-only reference — logs are on the host side)
    mounts.push({
      type: "directory",
      source: join(session.dir, "logs"),
      target: "/logs",
      writable: false,
    });

    // Env
    const env: Record<string, string> = {};
    for (const key of profile.envAllowlist) {
      const val = process.env[key];
      if (val !== undefined) env[key] = val;
    }

    return {
      mounts,
      env,
      cwd: "/workspace",
    };
  }

  // -------------------------------------------------------------------------
  // Execution lifecycle
  // -------------------------------------------------------------------------

  markExecutionStarted(
    session: Session,
    executionId: string,
    pid: number,
    profileName: string,
  ): Session {
    const now = new Date().toISOString();
    const record: SessionRecord = {
      ...session.record,
      state: "active",
      lastActiveAt: now,
      lastHeartbeat: now,
      activeExecution: {
        executionId,
        pid,
        startedAt: now,
        profileName,
      },
    };
    this._writeRecord(session.dir, record);
    return { record, dir: session.dir };
  }

  markExecutionFinished(session: Session): Session {
    const now = new Date().toISOString();
    const record: SessionRecord = {
      ...session.record,
      state: "idle",
      lastActiveAt: now,
      activeExecution: null,
    };
    this._writeRecord(session.dir, record);
    return { record, dir: session.dir };
  }

  // -------------------------------------------------------------------------
  // Maintenance
  // -------------------------------------------------------------------------

  cleanTmp(session: Session): void {
    const tmpDir = join(session.dir, "tmp");
    try {
      rmSync(tmpDir, { recursive: true, force: true });
      mkdirSync(tmpDir, { recursive: true });
    } catch {
      // Best-effort
    }
  }

  tombstone(session: Session): Session {
    // Close browser page if browser manager is wired
    if (this.browserManager) {
      this.browserManager.closePage(session.record.sessionId).catch(() => {});
    }
    const record: SessionRecord = {
      ...session.record,
      state: "tombstoned",
      activeExecution: null,
    };
    this._writeRecord(session.dir, record);
    return { record, dir: session.dir };
  }

  updateRecord(session: Session, updates: Partial<SessionRecord>): Session {
    const record: SessionRecord = { ...session.record, ...updates };
    this._writeRecord(session.dir, record);
    return { record, dir: session.dir };
  }

  // -------------------------------------------------------------------------
  // Private helpers
  // -------------------------------------------------------------------------

  private _writeRecord(dir: string, record: SessionRecord): void {
    const path = join(dir, RECORD_FILENAME);
    writeFileSync(path, JSON.stringify(record, null, 2), "utf8");
  }

  private _readRecord(dir: string): SessionRecord | null {
    const path = join(dir, RECORD_FILENAME);
    try {
      const raw = readFileSync(path, "utf8");
      return JSON.parse(raw) as SessionRecord;
    } catch {
      return null;
    }
  }

  /** Resolve the session directory from a session ID. */
  sessionDir(sessionId: string): string {
    return resolve(this.baseDir, sessionId);
  }
}
