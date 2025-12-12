"""NixOS Sandbox Python SDK Client"""

from dataclasses import dataclass
from typing import Optional, Iterator
import httpx


@dataclass
class ShellResult:
    stdout: str
    stderr: str
    exit_code: int
    duration_ms: float


@dataclass
class CodeResult:
    output: str
    error: str
    exit_code: int
    duration_ms: float


@dataclass
class FileEntry:
    name: str
    path: str
    type: str
    size: int
    modified: str


class SandboxClient:
    """Client for NixOS Sandbox API"""
    
    def __init__(self, base_url: str = "http://localhost:8080", timeout: float = 60.0):
        self.base_url = base_url.rstrip("/")
        self.client = httpx.Client(base_url=self.base_url, timeout=timeout)
        self.async_client = httpx.AsyncClient(base_url=self.base_url, timeout=timeout)
    
    def close(self):
        self.client.close()
    
    async def aclose(self):
        await self.async_client.aclose()
    
    # ========== Health & Info ==========
    
    def health(self) -> dict:
        return self.client.get("/health").json()
    
    def info(self) -> dict:
        return self.client.get("/sandbox/info").json()
    
    # ========== Shell ==========
    
    def shell(
        self,
        command: str,
        cwd: Optional[str] = None,
        timeout: int = 30,
        env: Optional[dict] = None,
    ) -> ShellResult:
        r = self.client.post("/shell/exec", json={
            "command": command,
            "cwd": cwd,
            "timeout": timeout,
            "env": env,
        })
        r.raise_for_status()
        d = r.json()
        return ShellResult(**d)
    
    def shell_stream(
        self,
        command: str,
        cwd: Optional[str] = None,
    ) -> Iterator[str]:
        with self.client.stream("POST", "/shell/stream", json={
            "command": command,
            "cwd": cwd,
        }) as r:
            for line in r.iter_lines():
                if line.startswith("data: "):
                    yield line[6:]
    
    # ========== Code Execution ==========
    
    def run_code(
        self,
        code: str,
        language: str = "python",
        timeout: int = 30,
    ) -> CodeResult:
        r = self.client.post("/code/execute", json={
            "code": code,
            "language": language,
            "timeout": timeout,
        })
        r.raise_for_status()
        return CodeResult(**r.json())
    
    def run_python(self, code: str, timeout: int = 30) -> CodeResult:
        return self.run_code(code, "python", timeout)
    
    def run_js(self, code: str, timeout: int = 30) -> CodeResult:
        return self.run_code(code, "javascript", timeout)
    
    # ========== Files ==========
    
    def read_file(self, path: str, encoding: str = "utf-8") -> str:
        r = self.client.get("/file/read", params={"path": path, "encoding": encoding})
        r.raise_for_status()
        return r.json()["content"]
    
    def write_file(self, path: str, content: str, mode: str = "644") -> dict:
        r = self.client.post("/file/write", json={
            "path": path,
            "content": content,
            "mode": mode,
        })
        r.raise_for_status()
        return r.json()
    
    def list_files(self, path: str = ".", recursive: bool = False) -> list[FileEntry]:
        r = self.client.get("/file/list", params={"path": path, "recursive": recursive})
        r.raise_for_status()
        return [FileEntry(**e) for e in r.json()["entries"]]
    
    def upload_file(self, local_path: str, remote_path: str) -> dict:
        with open(local_path, "rb") as f:
            r = self.client.post(
                "/file/upload",
                files={"file": f},
                data={"path": remote_path},
            )
        r.raise_for_status()
        return r.json()
    
    def download_file(self, path: str) -> bytes:
        r = self.client.get("/file/download", params={"path": path})
        r.raise_for_status()
        return r.content
    
    # ========== Browser ==========
    
    def browser_launch(self, headless: bool = False) -> dict:
        r = self.client.post("/browser/launch", params={"headless": headless})
        r.raise_for_status()
        return r.json()
    
    def browser_navigate(self, url: str, wait_until: str = "load") -> dict:
        r = self.client.post("/browser/navigate", json={
            "url": url,
            "wait_until": wait_until,
        })
        r.raise_for_status()
        return r.json()
    
    def browser_screenshot(self, full_page: bool = False) -> bytes:
        r = self.client.get("/browser/screenshot", params={"full_page": full_page})
        r.raise_for_status()
        return r.content
    
    def browser_click(
        self,
        selector: Optional[str] = None,
        x: Optional[int] = None,
        y: Optional[int] = None,
    ) -> dict:
        r = self.client.post("/browser/click", json={
            "selector": selector,
            "x": x,
            "y": y,
        })
        r.raise_for_status()
        return r.json()
    
    def browser_type(
        self,
        text: str,
        selector: Optional[str] = None,
    ) -> dict:
        r = self.client.post("/browser/type", json={
            "text": text,
            "selector": selector,
        })
        r.raise_for_status()
        return r.json()
    
    def browser_eval(self, script: str):
        r = self.client.post("/browser/evaluate", json={"script": script})
        r.raise_for_status()
        return r.json()["result"]
    
    def browser_close(self) -> dict:
        r = self.client.post("/browser/close")
        r.raise_for_status()
        return r.json()
    
    # ========== Screen (Desktop) ==========
    
    def screen_screenshot(self) -> bytes:
        r = self.client.get("/screen/screenshot")
        r.raise_for_status()
        return r.content
    
    def screen_click(self, x: int, y: int, button: str = "left") -> dict:
        r = self.client.post("/screen/mouse", json={
            "action": "click",
            "x": x,
            "y": y,
            "button": button,
        })
        r.raise_for_status()
        return r.json()
    
    def screen_type(self, text: str) -> dict:
        r = self.client.post("/screen/keyboard", json={"text": text})
        r.raise_for_status()
        return r.json()
    
    def screen_key(self, key: str, modifiers: Optional[list[str]] = None) -> dict:
        r = self.client.post("/screen/keyboard", json={
            "key": key,
            "modifiers": modifiers,
        })
        r.raise_for_status()
        return r.json()


# ========== Usage Example ==========

if __name__ == "__main__":
    sandbox = SandboxClient()
    
    # Check health
    print("Health:", sandbox.health())
    
    # Run shell command
    result = sandbox.shell("echo 'Hello from NixOS sandbox!'")
    print("Shell:", result.stdout)
    
    # Execute Python code
    code_result = sandbox.run_python("""
import sys
print(f"Python {sys.version}")
print(2 + 2)
""")
    print("Python output:", code_result.output)
    
    # File operations
    sandbox.write_file("test.txt", "Hello, sandbox!")
    content = sandbox.read_file("test.txt")
    print("File content:", content)
    
    # Browser automation
    sandbox.browser_launch()
    sandbox.browser_navigate("https://example.com")
    screenshot = sandbox.browser_screenshot()
    print(f"Screenshot: {len(screenshot)} bytes")
    sandbox.browser_close()
    
    sandbox.close()