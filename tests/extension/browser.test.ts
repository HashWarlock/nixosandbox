import { describe, it, expect, afterAll } from "vitest";
import { chromium } from "playwright-core";

// Import the BrowserManager from the extension source
import { BrowserManager } from "../../packages/pi-sandbox-extension/src/browser.js";

// Skip all tests if no browser is available
const hasBrowser = await (async () => {
  try {
    const b = await chromium.launch({ headless: true });
    await b.close();
    return true;
  } catch {
    return false;
  }
})();

describe.skipIf(!hasBrowser)("BrowserManager", () => {
  const manager = new BrowserManager();

  afterAll(async () => {
    await manager.shutdown();
  });

  it("getOrCreatePage returns a page for a session", async () => {
    const page = await manager.getOrCreatePage("session-1");
    expect(page).toBeDefined();
    expect(page.isClosed()).toBe(false);
  });

  it("getOrCreatePage returns the SAME page for the same session", async () => {
    const page1 = await manager.getOrCreatePage("session-2");
    const page2 = await manager.getOrCreatePage("session-2");
    expect(page1).toBe(page2);
  });

  it("closePage closes the page and removes it from the map", async () => {
    const page = await manager.getOrCreatePage("session-close");
    expect(page.isClosed()).toBe(false);
    await manager.closePage("session-close");
    expect(page.isClosed()).toBe(true);
    // A new call should create a fresh page
    const page2 = await manager.getOrCreatePage("session-close");
    expect(page2).not.toBe(page);
  });

  it("execute goto navigates and returns content", async () => {
    const result = await manager.execute("session-goto", "goto", {
      url: "data:text/html,<html><head><title>Test</title></head><body>Hello World</body></html>",
    });
    expect(result).toContain("title: Test");
    expect(result).toContain("Hello World");
  });

  it("execute screenshot returns base64 PNG", async () => {
    await manager.execute("session-ss", "goto", {
      url: "data:text/html,<html><body>Screenshot Test</body></html>",
    });
    const result = await manager.execute("session-ss", "screenshot", {});
    // PNG base64 starts with iVBOR
    expect(result.startsWith("iVBOR")).toBe(true);
  });

  it("execute evaluate runs JavaScript and returns result", async () => {
    await manager.execute("session-eval", "goto", {
      url: "data:text/html,<html><body></body></html>",
    });
    const result = await manager.execute("session-eval", "evaluate", {
      script: "1 + 2",
    });
    expect(result).toBe("3");
  });

  it("execute click clicks an element", async () => {
    await manager.execute("session-click", "goto", {
      url: 'data:text/html,<html><body><button id="btn" onclick="document.title=\'clicked\'">Click me</button></body></html>',
    });
    await manager.execute("session-click", "click", { selector: "#btn" });
    const title = await manager.execute("session-click", "evaluate", {
      script: "document.title",
    });
    expect(title).toBe('"clicked"');
  });

  it("execute type fills an input", async () => {
    await manager.execute("session-type", "goto", {
      url: 'data:text/html,<html><body><input id="inp" /></body></html>',
    });
    await manager.execute("session-type", "type", {
      selector: "#inp",
      text: "hello",
    });
    const value = await manager.execute("session-type", "evaluate", {
      script: 'document.querySelector("#inp").value',
    });
    expect(value).toBe('"hello"');
  });

  it("execute close closes the page", async () => {
    await manager.getOrCreatePage("session-close2");
    const result = await manager.execute("session-close2", "close", {});
    expect(result).toBe("Browser page closed.");
  });

  it("execute throws on unknown action", async () => {
    await expect(
      manager.execute("session-err", "invalid" as any, {}),
    ).rejects.toThrow("Unknown browser action");
  });

  it("shutdown closes all pages and the browser", async () => {
    await manager.getOrCreatePage("session-shutdown-1");
    await manager.getOrCreatePage("session-shutdown-2");
    await manager.shutdown();
    // After shutdown, a new call should re-launch
    const page = await manager.getOrCreatePage("session-after-shutdown");
    expect(page.isClosed()).toBe(false);
    await manager.shutdown();
  });
});
