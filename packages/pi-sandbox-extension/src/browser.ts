/**
 * Browser Manager
 *
 * Manages a shared Playwright browser instance with session-scoped pages.
 * Lazy-initialized on first use. Each sandbox session gets one persistent
 * page that maintains state across navigation/click/type calls.
 */

import type { Browser, BrowserContext, Page } from "playwright-core";
import { chromium } from "playwright-core";

export class BrowserManager {
  private browser: Browser | null = null;
  private context: BrowserContext | null = null;
  private pages: Map<string, Page> = new Map();
  private launchPromise: Promise<BrowserContext> | null = null;

  /**
   * Launch the browser if not already running.
   * Uses PLAYWRIGHT_CHROMIUM_PATH env var or system chromium.
   * Guards against concurrent callers racing to launch two browsers.
   */
  private ensureBrowser(): Promise<BrowserContext> {
    if (!this.launchPromise) {
      this.launchPromise = this._doLaunch();
    }
    return this.launchPromise;
  }

  private async _doLaunch(): Promise<BrowserContext> {
    const executablePath = process.env.PLAYWRIGHT_CHROMIUM_PATH || undefined;
    this.browser = await chromium.launch({
      headless: true,
      executablePath,
    });
    this.context = await this.browser.newContext();
    return this.context;
  }

  /**
   * Get or create a page for the given session.
   */
  async getOrCreatePage(sessionId: string): Promise<Page> {
    const existing = this.pages.get(sessionId);
    if (existing && !existing.isClosed()) return existing;

    const ctx = await this.ensureBrowser();
    const page = await ctx.newPage();
    this.pages.set(sessionId, page);
    return page;
  }

  /**
   * Close the page for a specific session (e.g., on session teardown).
   */
  async closePage(sessionId: string): Promise<void> {
    const page = this.pages.get(sessionId);
    if (page && !page.isClosed()) {
      await page.close();
    }
    this.pages.delete(sessionId);
  }

  /**
   * Execute a browser action for a session.
   */
  async execute(
    sessionId: string,
    action: string,
    params: {
      url?: string;
      selector?: string;
      text?: string;
      script?: string;
    },
  ): Promise<string> {
    if (action === "close") {
      await this.closePage(sessionId);
      return "Browser page closed.";
    }

    const page = await this.getOrCreatePage(sessionId);

    switch (action) {
      case "goto": {
        if (!params.url) throw new Error("url is required for goto action");
        const response = await page.goto(params.url, {
          waitUntil: "domcontentloaded",
        });
        const title = await page.title();
        const textContent = await page.evaluate(() => {
          const body = document.body;
          return body ? body.innerText.slice(0, 4000) : "";
        });
        const status = response?.status() ?? 0;
        return [
          `url: ${page.url()}`,
          `status: ${status}`,
          `title: ${title}`,
          "--- content ---",
          textContent,
        ].join("\n");
      }

      case "screenshot": {
        const buffer = await page.screenshot({ type: "png" });
        return buffer.toString("base64");
      }

      case "evaluate": {
        if (!params.script)
          throw new Error("script is required for evaluate action");
        const result = await page.evaluate(params.script);
        return JSON.stringify(result);
      }

      case "click": {
        if (!params.selector)
          throw new Error("selector is required for click action");
        await page.click(params.selector);
        return `Clicked: ${params.selector}`;
      }

      case "type": {
        if (!params.selector)
          throw new Error("selector is required for type action");
        if (!params.text)
          throw new Error("text is required for type action");
        await page.fill(params.selector, params.text);
        return `Typed into: ${params.selector}`;
      }

      default:
        throw new Error(
          `Unknown browser action: "${action}". Valid: goto, screenshot, evaluate, click, type, close`,
        );
    }
  }

  /**
   * Shut down the browser entirely. Called on extension teardown.
   */
  async shutdown(): Promise<void> {
    for (const [, page] of this.pages) {
      if (!page.isClosed()) {
        await page.close();
      }
    }
    this.pages.clear();
    if (this.context) {
      await this.context.close();
      this.context = null;
    }
    if (this.browser) {
      await this.browser.close();
      this.browser = null;
    }
    this.launchPromise = null;
  }
}
