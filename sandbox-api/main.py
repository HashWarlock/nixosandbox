#!/usr/bin/env python3
"""NixOS Sandbox Control API - Main Server"""

import asyncio
import base64
import os
import subprocess
import tempfile
import time
from contextlib import asynccontextmanager
from datetime import datetime
from pathlib import Path
from typing import Optional

import aiofiles
import psutil
from fastapi import FastAPI, HTTPException, Query, UploadFile, File, Form
from fastapi.responses import StreamingResponse, Response
from pydantic import BaseModel
from playwright.async_api import async_playwright, Browser, Page

# ============================================================================
# Configuration
# ============================================================================

WORKSPACE = os.getenv("WORKSPACE", "/home/sandbox/workspace")
DISPLAY = os.getenv("DISPLAY", ":99")
CDP_PORT = int(os.getenv("CDP_PORT", "9222"))
START_TIME = time.time()

# ============================================================================
# Models
# ============================================================================

class ShellExecRequest(BaseModel):
    command: str
    cwd: Optional[str] = None
    timeout: int = 30
    env: Optional[dict[str, str]] = None

class ShellExecResponse(BaseModel):
    stdout: str
    stderr: str
    exit_code: int
    duration_ms: float

class CodeExecRequest(BaseModel):
    code: str
    language: str
    timeout: int = 30

class CodeExecResponse(BaseModel):
    output: str
    error: str
    exit_code: int
    duration_ms: float

class FileWriteRequest(BaseModel):
    path: str
    content: str
    mode: str = "644"

class BrowserNavigateRequest(BaseModel):
    url: str
    wait_until: str = "load"

class BrowserClickRequest(BaseModel):
    selector: Optional[str] = None
    x: Optional[int] = None
    y: Optional[int] = None
    button: str = "left"

class BrowserTypeRequest(BaseModel):
    selector: Optional[str] = None
    text: str
    delay: int = 0

class BrowserEvalRequest(BaseModel):
    script: str

class MouseActionRequest(BaseModel):
    action: str
    x: int
    y: int
    button: str = "left"

class KeyboardActionRequest(BaseModel):
    text: Optional[str] = None
    key: Optional[str] = None
    modifiers: Optional[list[str]] = None

# ============================================================================
# Browser Manager
# ============================================================================

class BrowserManager:
    def __init__(self):
        self.playwright = None
        self.browser: Optional[Browser] = None
        self.page: Optional[Page] = None
    
    async def launch(self, headless: bool = False):
        if not self.playwright:
            self.playwright = await async_playwright().start()
        
        if self.browser:
            await self.browser.close()
        
        self.browser = await self.playwright.chromium.launch(
            headless=headless,
            args=[
                f"--remote-debugging-port={CDP_PORT}",
                "--no-sandbox",
                "--disable-setuid-sandbox",
                "--disable-dev-shm-usage",
                "--disable-gpu",
            ]
        )
        self.page = await self.browser.new_page()
        return {
            "cdp_url": f"http://localhost:{CDP_PORT}",
            "ws_endpoint": self.browser.contexts[0].pages[0].url if self.browser.contexts else None
        }
    
    async def ensure_page(self) -> Page:
        if not self.page:
            await self.launch()
        return self.page
    
    async def close(self):
        if self.browser:
            await self.browser.close()
            self.browser = None
            self.page = None

browser_mgr = BrowserManager()

# ============================================================================
# App Lifecycle
# ============================================================================

@asynccontextmanager
async def lifespan(app: FastAPI):
    os.makedirs(WORKSPACE, exist_ok=True)
    yield
    await browser_mgr.close()

app = FastAPI(
    title="NixOS Sandbox API",
    version="1.0.0",
    lifespan=lifespan
)

# ============================================================================
# Health & Info
# ============================================================================

@app.get("/health")
async def health_check():
    return {
        "status": "healthy",
        "uptime": time.time() - START_TIME,
        "services": {
            "display": os.path.exists("/tmp/.X11-unix/X99"),
            "browser": browser_mgr.browser is not None,
        }
    }

@app.get("/sandbox/info")
async def sandbox_info():
    return {
        "hostname": os.uname().nodename,
        "workspace": WORKSPACE,
        "display": DISPLAY,
        "cdp_url": f"http://localhost:{CDP_PORT}",
        "vnc_url": "vnc://localhost:5900",
    }

# ============================================================================
# Shell Operations
# ============================================================================

@app.post("/shell/exec", response_model=ShellExecResponse)
async def exec_command(req: ShellExecRequest):
    start = time.time()
    env = {**os.environ, **(req.env or {})}
    cwd = req.cwd or WORKSPACE
    
    try:
        proc = await asyncio.create_subprocess_shell(
            req.command,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=cwd,
            env=env,
        )
        stdout, stderr = await asyncio.wait_for(
            proc.communicate(), timeout=req.timeout
        )
        return ShellExecResponse(
            stdout=stdout.decode("utf-8", errors="replace"),
            stderr=stderr.decode("utf-8", errors="replace"),
            exit_code=proc.returncode or 0,
            duration_ms=(time.time() - start) * 1000,
        )
    except asyncio.TimeoutError:
        raise HTTPException(408, "Command timed out")
    except Exception as e:
        raise HTTPException(500, str(e))

@app.post("/shell/stream")
async def exec_command_stream(req: ShellExecRequest):
    async def generate():
        proc = await asyncio.create_subprocess_shell(
            req.command,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
            cwd=req.cwd or WORKSPACE,
        )
        async for line in proc.stdout:
            yield f"data: {line.decode('utf-8', errors='replace')}\n\n"
        yield f"data: [exit_code:{proc.returncode}]\n\n"
    
    return StreamingResponse(generate(), media_type="text/event-stream")

# ============================================================================
# Code Execution
# ============================================================================

LANG_CONFIG = {
    "python": {"ext": ".py", "cmd": "python3"},
    "javascript": {"ext": ".js", "cmd": "node"},
    "typescript": {"ext": ".ts", "cmd": "npx tsx"},
    "go": {"ext": ".go", "cmd": "go run"},
    "rust": {"ext": ".rs", "cmd": "rustc -o /tmp/rust_out && /tmp/rust_out"},
    "bash": {"ext": ".sh", "cmd": "bash"},
}

@app.post("/code/execute", response_model=CodeExecResponse)
async def execute_code(req: CodeExecRequest):
    lang = req.language.lower()
    if lang not in LANG_CONFIG:
        raise HTTPException(400, f"Unsupported language: {lang}")
    
    cfg = LANG_CONFIG[lang]
    start = time.time()
    
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=cfg["ext"], delete=False
    ) as f:
        f.write(req.code)
        f.flush()
        tmp_path = f.name
    
    try:
        cmd = f"{cfg['cmd']} {tmp_path}"
        proc = await asyncio.create_subprocess_shell(
            cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=WORKSPACE,
        )
        stdout, stderr = await asyncio.wait_for(
            proc.communicate(), timeout=req.timeout
        )
        return CodeExecResponse(
            output=stdout.decode("utf-8", errors="replace"),
            error=stderr.decode("utf-8", errors="replace"),
            exit_code=proc.returncode or 0,
            duration_ms=(time.time() - start) * 1000,
        )
    except asyncio.TimeoutError:
        raise HTTPException(408, "Execution timed out")
    finally:
        os.unlink(tmp_path)

# ============================================================================
# File Operations
# ============================================================================

@app.get("/file/read")
async def read_file(path: str = Query(...), encoding: str = "utf-8"):
    full_path = Path(path) if path.startswith("/") else Path(WORKSPACE) / path
    if not full_path.exists():
        raise HTTPException(404, "File not found")
    
    async with aiofiles.open(full_path, "r", encoding=encoding) as f:
        content = await f.read()
    
    return {
        "content": content,
        "size": full_path.stat().st_size,
        "mime_type": "text/plain",
    }

@app.post("/file/write")
async def write_file(req: FileWriteRequest):
    full_path = Path(req.path) if req.path.startswith("/") else Path(WORKSPACE) / req.path
    full_path.parent.mkdir(parents=True, exist_ok=True)
    
    async with aiofiles.open(full_path, "w") as f:
        await f.write(req.content)
    
    os.chmod(full_path, int(req.mode, 8))
    return {"path": str(full_path), "size": full_path.stat().st_size}

@app.get("/file/list")
async def list_files(path: str = Query(...), recursive: bool = False):
    full_path = Path(path) if path.startswith("/") else Path(WORKSPACE) / path
    if not full_path.exists():
        raise HTTPException(404, "Path not found")
    
    entries = []
    iterator = full_path.rglob("*") if recursive else full_path.iterdir()
    
    for p in iterator:
        stat = p.stat()
        entries.append({
            "name": p.name,
            "path": str(p),
            "type": "directory" if p.is_dir() else "file",
            "size": stat.st_size,
            "modified": datetime.fromtimestamp(stat.st_mtime).isoformat(),
        })
    
    return {"path": str(full_path), "entries": entries}

@app.post("/file/upload")
async def upload_file(file: UploadFile = File(...), path: str = Form(...)):
    full_path = Path(path) if path.startswith("/") else Path(WORKSPACE) / path
    full_path.parent.mkdir(parents=True, exist_ok=True)
    
    async with aiofiles.open(full_path, "wb") as f:
        content = await file.read()
        await f.write(content)
    
    return {"path": str(full_path), "size": len(content)}

@app.get("/file/download")
async def download_file(path: str = Query(...)):
    full_path = Path(path) if path.startswith("/") else Path(WORKSPACE) / path
    if not full_path.exists():
        raise HTTPException(404, "File not found")
    
    async with aiofiles.open(full_path, "rb") as f:
        content = await f.read()
    
    return Response(
        content=content,
        media_type="application/octet-stream",
        headers={"Content-Disposition": f"attachment; filename={full_path.name}"}
    )

# ============================================================================
# Browser Operations
# ============================================================================

@app.post("/browser/launch")
async def launch_browser(headless: bool = False):
    return await browser_mgr.launch(headless=headless)

@app.post("/browser/navigate")
async def browser_navigate(req: BrowserNavigateRequest):
    page = await browser_mgr.ensure_page()
    await page.goto(req.url, wait_until=req.wait_until)
    return {"url": page.url, "title": await page.title()}

@app.get("/browser/screenshot")
async def browser_screenshot(full_page: bool = False, format: str = "png"):
    page = await browser_mgr.ensure_page()
    data = await page.screenshot(full_page=full_page, type=format)
    
    return Response(content=data, media_type=f"image/{format}")

@app.post("/browser/click")
async def browser_click(req: BrowserClickRequest):
    page = await browser_mgr.ensure_page()
    if req.selector:
        await page.click(req.selector, button=req.button)
    elif req.x is not None and req.y is not None:
        await page.mouse.click(req.x, req.y, button=req.button)
    else:
        raise HTTPException(400, "Provide selector or coordinates")
    return {"status": "clicked"}

@app.post("/browser/type")
async def browser_type(req: BrowserTypeRequest):
    page = await browser_mgr.ensure_page()
    if req.selector:
        await page.fill(req.selector, req.text)
    else:
        await page.keyboard.type(req.text, delay=req.delay)
    return {"status": "typed"}

@app.post("/browser/evaluate")
async def browser_evaluate(req: BrowserEvalRequest):
    page = await browser_mgr.ensure_page()
    result = await page.evaluate(req.script)
    return {"result": result}

@app.post("/browser/close")
async def close_browser():
    await browser_mgr.close()
    return {"status": "closed"}

# ============================================================================
# Screen Operations (Desktop via xdotool/scrot)
# ============================================================================

@app.get("/screen/screenshot")
async def screen_screenshot():
    with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
        tmp_path = f.name
    
    try:
        proc = await asyncio.create_subprocess_exec(
            "scrot", "-o", tmp_path,
            env={**os.environ, "DISPLAY": DISPLAY}
        )
        await proc.wait()
        
        async with aiofiles.open(tmp_path, "rb") as f:
            data = await f.read()
        
        return Response(content=data, media_type="image/png")
    finally:
        if os.path.exists(tmp_path):
            os.unlink(tmp_path)

@app.post("/screen/mouse")
async def screen_mouse(req: MouseActionRequest):
    env = {**os.environ, "DISPLAY": DISPLAY}
    
    if req.action == "move":
        cmd = ["xdotool", "mousemove", str(req.x), str(req.y)]
    elif req.action == "click":
        btn = {"left": "1", "middle": "2", "right": "3"}[req.button]
        cmd = ["xdotool", "mousemove", str(req.x), str(req.y), "click", btn]
    elif req.action == "double_click":
        btn = {"left": "1", "middle": "2", "right": "3"}[req.button]
        cmd = ["xdotool", "mousemove", str(req.x), str(req.y), "click", "--repeat", "2", btn]
    else:
        raise HTTPException(400, f"Unknown action: {req.action}")
    
    proc = await asyncio.create_subprocess_exec(*cmd, env=env)
    await proc.wait()
    return {"status": "ok"}

@app.post("/screen/keyboard")
async def screen_keyboard(req: KeyboardActionRequest):
    env = {**os.environ, "DISPLAY": DISPLAY}
    
    if req.text:
        cmd = ["xdotool", "type", "--", req.text]
    elif req.key:
        key = req.key
        if req.modifiers:
            key = "+".join(req.modifiers) + "+" + key
        cmd = ["xdotool", "key", key]
    else:
        raise HTTPException(400, "Provide text or key")
    
    proc = await asyncio.create_subprocess_exec(*cmd, env=env)
    await proc.wait()
    return {"status": "ok"}

# ============================================================================
# Main
# ============================================================================

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        app,
        host=os.getenv("SANDBOX_API_HOST", "0.0.0.0"),
        port=int(os.getenv("SANDBOX_API_PORT", "8080")),
    )