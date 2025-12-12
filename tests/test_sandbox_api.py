#!/usr/bin/env python3
"""
NixOS Sandbox API Test Suite

Comprehensive tests for validating sandbox API functionality including
multi-turn task execution scenarios.

Usage:
    # Run all tests
    pytest test_sandbox_api.py -v --api-url http://localhost:8080

    # Run specific test categories
    pytest test_sandbox_api.py -v --api-url http://localhost:8080 -k "health"
    pytest test_sandbox_api.py -v --api-url http://localhost:8080 -k "browser"
    pytest test_sandbox_api.py -v --api-url http://localhost:8080 -k "multi_turn"

    # Run with output
    pytest test_sandbox_api.py -v -s --api-url http://localhost:8080
"""

import asyncio
import base64
import io
import os
import time
from typing import Optional

import httpx
import pytest
from PIL import Image


# Fixtures are defined in conftest.py


# ============================================================================
# Health & Info Tests
# ============================================================================


class TestHealthAndInfo:
    """Basic health and info endpoint tests."""

    def test_health_check(self, client):
        """Verify health endpoint returns healthy status."""
        resp = client.get("/health")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "healthy"
        assert "uptime" in data
        assert "services" in data

    def test_health_services(self, client):
        """Verify health endpoint reports service status."""
        resp = client.get("/health")
        data = resp.json()
        services = data["services"]
        assert "display" in services
        assert "browser" in services

    def test_sandbox_info(self, client):
        """Verify sandbox info endpoint."""
        resp = client.get("/sandbox/info")
        assert resp.status_code == 200
        data = resp.json()
        assert "hostname" in data
        assert "workspace" in data
        assert "display" in data
        assert "cdp_url" in data
        assert "vnc_url" in data


# ============================================================================
# Shell Execution Tests
# ============================================================================


class TestShellExecution:
    """Shell command execution tests."""

    def test_simple_command(self, client):
        """Execute a simple echo command."""
        resp = client.post("/shell/exec", json={"command": "echo 'Hello World'"})
        assert resp.status_code == 200
        data = resp.json()
        assert "Hello World" in data["stdout"]
        assert data["exit_code"] == 0

    def test_command_with_env(self, client):
        """Execute command with custom environment variables."""
        resp = client.post(
            "/shell/exec",
            json={"command": "echo $MY_VAR", "env": {"MY_VAR": "test_value"}},
        )
        assert resp.status_code == 200
        assert "test_value" in resp.json()["stdout"]

    def test_command_stderr(self, client):
        """Verify stderr is captured."""
        resp = client.post("/shell/exec", json={"command": "echo 'error' >&2"})
        assert resp.status_code == 200
        assert "error" in resp.json()["stderr"]

    def test_command_exit_code(self, client):
        """Verify exit codes are captured."""
        resp = client.post("/shell/exec", json={"command": "exit 42"})
        assert resp.status_code == 200
        assert resp.json()["exit_code"] == 42

    def test_command_timeout(self, client):
        """Verify commands timeout correctly."""
        resp = client.post("/shell/exec", json={"command": "sleep 10", "timeout": 1})
        assert resp.status_code == 408

    def test_complex_pipeline(self, client):
        """Execute a complex shell pipeline."""
        resp = client.post(
            "/shell/exec",
            json={"command": "echo -e 'line1\\nline2\\nline3' | grep line | wc -l"},
        )
        assert resp.status_code == 200
        assert "3" in resp.json()["stdout"]


# ============================================================================
# Code Execution Tests
# ============================================================================


class TestCodeExecution:
    """Code execution tests for various languages."""

    def test_python_execution(self, client):
        """Execute Python code."""
        code = """
import sys
print("Python version:", sys.version_info.major)
print("Hello from Python!")
"""
        resp = client.post(
            "/code/execute", json={"code": code, "language": "python"}
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "Hello from Python!" in data["output"]
        assert data["exit_code"] == 0

    def test_python_with_error(self, client):
        """Execute Python code that raises an error."""
        code = "raise ValueError('Test error')"
        resp = client.post(
            "/code/execute", json={"code": code, "language": "python"}
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["exit_code"] != 0
        assert "ValueError" in data["error"]

    def test_bash_execution(self, client):
        """Execute bash script."""
        code = """
#!/bin/bash
for i in 1 2 3; do
    echo "Number: $i"
done
"""
        resp = client.post("/code/execute", json={"code": code, "language": "bash"})
        assert resp.status_code == 200
        data = resp.json()
        assert "Number: 1" in data["output"]
        assert "Number: 3" in data["output"]

    def test_unsupported_language(self, client):
        """Verify unsupported language returns error."""
        resp = client.post(
            "/code/execute", json={"code": "print('test')", "language": "cobol"}
        )
        assert resp.status_code == 400


# ============================================================================
# File Operation Tests
# ============================================================================


class TestFileOperations:
    """File system operation tests."""

    def test_write_and_read_file(self, client):
        """Write a file and read it back."""
        test_content = "Hello, this is a test file!\nWith multiple lines."
        test_path = "test_file.txt"

        # Write
        resp = client.post(
            "/file/write", json={"path": test_path, "content": test_content}
        )
        assert resp.status_code == 200
        write_data = resp.json()
        assert "path" in write_data

        # Read
        resp = client.get("/file/read", params={"path": test_path})
        assert resp.status_code == 200
        read_data = resp.json()
        assert read_data["content"] == test_content

    def test_list_files(self, client):
        """List files in a directory."""
        # Create a test file first
        client.post("/file/write", json={"path": "list_test.txt", "content": "test"})

        resp = client.get("/file/list", params={"path": "."})
        assert resp.status_code == 200
        data = resp.json()
        assert "entries" in data
        assert len(data["entries"]) > 0

    def test_read_nonexistent_file(self, client):
        """Verify reading nonexistent file returns 404."""
        resp = client.get("/file/read", params={"path": "nonexistent_file_12345.txt"})
        assert resp.status_code == 404

    def test_nested_directory_write(self, client):
        """Write file in nested directory (should create parents)."""
        resp = client.post(
            "/file/write",
            json={"path": "nested/dir/test.txt", "content": "nested content"},
        )
        assert resp.status_code == 200

        resp = client.get("/file/read", params={"path": "nested/dir/test.txt"})
        assert resp.status_code == 200
        assert resp.json()["content"] == "nested content"


# ============================================================================
# Browser Tests
# ============================================================================


class TestBrowser:
    """Browser automation tests."""

    def test_browser_launch(self, client):
        """Launch browser and verify."""
        resp = client.post("/browser/launch", params={"headless": False})
        assert resp.status_code == 200
        data = resp.json()
        assert "cdp_url" in data

    def test_browser_navigate(self, client):
        """Navigate to a URL."""
        # Ensure browser is launched
        client.post("/browser/launch")

        resp = client.post(
            "/browser/navigate",
            json={"url": "https://example.com", "wait_until": "load"},
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "example.com" in data["url"].lower() or "Example" in data["title"]

    def test_browser_screenshot(self, client):
        """Take a browser screenshot."""
        # Navigate first
        client.post("/browser/launch")
        client.post("/browser/navigate", json={"url": "https://example.com"})

        resp = client.get("/browser/screenshot")
        assert resp.status_code == 200
        assert resp.headers["content-type"] == "image/png"

        # Verify it's a valid image
        img = Image.open(io.BytesIO(resp.content))
        assert img.width > 0
        assert img.height > 0

    def test_browser_evaluate(self, client):
        """Evaluate JavaScript in browser."""
        client.post("/browser/launch")
        client.post("/browser/navigate", json={"url": "https://example.com"})

        resp = client.post(
            "/browser/evaluate", json={"script": "document.title"}
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "result" in data

    def test_browser_close(self, client):
        """Close browser."""
        client.post("/browser/launch")
        resp = client.post("/browser/close")
        assert resp.status_code == 200
        assert resp.json()["status"] == "closed"


# ============================================================================
# Screen/Desktop Tests
# ============================================================================


class TestScreen:
    """Desktop/screen operation tests."""

    def test_screen_screenshot(self, client):
        """Take a desktop screenshot."""
        resp = client.get("/screen/screenshot")
        assert resp.status_code == 200
        assert resp.headers["content-type"] == "image/png"

        # Verify it's a valid image
        img = Image.open(io.BytesIO(resp.content))
        assert img.width > 0
        assert img.height > 0

    def test_screen_mouse_move(self, client):
        """Move mouse cursor."""
        resp = client.post(
            "/screen/mouse", json={"action": "move", "x": 100, "y": 100}
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "ok"

    def test_screen_mouse_click(self, client):
        """Click mouse button."""
        resp = client.post(
            "/screen/mouse", json={"action": "click", "x": 100, "y": 100}
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "ok"

    def test_screen_keyboard_type(self, client):
        """Type text via keyboard."""
        resp = client.post("/screen/keyboard", json={"text": "Hello"})
        assert resp.status_code == 200
        assert resp.json()["status"] == "ok"

    def test_screen_keyboard_key(self, client):
        """Press a key."""
        resp = client.post("/screen/keyboard", json={"key": "Return"})
        assert resp.status_code == 200
        assert resp.json()["status"] == "ok"

    def test_screen_recording(self, client):
        """Test screen recording start/stop/download."""
        # Check initial status
        resp = client.get("/screen/record/status")
        assert resp.status_code == 200
        status = resp.json()
        assert status["recording"] is False

        # Start recording
        resp = client.post(
            "/screen/record/start",
            json={"output_path": "/tmp/test_recording.mp4", "fps": 10}
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "recording"
        assert data["pid"] is not None

        # Check recording status
        resp = client.get("/screen/record/status")
        assert resp.status_code == 200
        status = resp.json()
        assert status["recording"] is True

        # Do something visible (move mouse around)
        for i in range(3):
            client.post("/screen/mouse", json={"action": "move", "x": 100 + i * 50, "y": 100})
            time.sleep(0.5)

        # Stop recording
        resp = client.post("/screen/record/stop")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "stopped"
        assert data.get("size_bytes", 0) > 0, f"Recording failed: {data.get('error', 'unknown')}"

        # Download recording
        resp = client.get("/screen/record/download", params={"path": "/tmp/test_recording.mp4"})
        assert resp.status_code == 200
        assert len(resp.content) > 0
        assert resp.headers["content-type"] == "video/mp4"


# ============================================================================
# Multi-Turn Task Tests
# ============================================================================


class TestMultiTurnTasks:
    """
    Multi-turn task execution tests.
    These tests simulate realistic workflows that require multiple API calls.
    """

    def test_create_and_execute_script(self, client):
        """
        Multi-turn: Create a script file, execute it, verify output.
        Simulates a code development workflow.
        """
        # Step 1: Write a Python script
        script = """
import json
data = {"message": "Hello from script!", "count": 42}
print(json.dumps(data))
"""
        resp = client.post(
            "/file/write", json={"path": "test_script.py", "content": script}
        )
        assert resp.status_code == 200

        # Step 2: Execute the script via shell
        resp = client.post(
            "/shell/exec", json={"command": "python3 test_script.py"}
        )
        assert resp.status_code == 200
        import json

        output = json.loads(resp.json()["stdout"])
        assert output["message"] == "Hello from script!"
        assert output["count"] == 42

        # Step 3: Clean up
        resp = client.post("/shell/exec", json={"command": "rm test_script.py"})
        assert resp.status_code == 200

    def test_web_form_interaction(self, client):
        """
        Multi-turn: Navigate to a page, interact with form elements.
        Simulates web automation workflow.
        """
        # Step 1: Launch browser
        resp = client.post("/browser/launch")
        assert resp.status_code == 200

        # Step 2: Navigate to a simple test page
        resp = client.post(
            "/browser/navigate",
            json={"url": "https://example.com", "wait_until": "load"},
        )
        assert resp.status_code == 200

        # Step 3: Take screenshot to verify page loaded
        resp = client.get("/browser/screenshot")
        assert resp.status_code == 200
        img = Image.open(io.BytesIO(resp.content))
        assert img.width > 0

        # Step 4: Get page title via JavaScript
        resp = client.post("/browser/evaluate", json={"script": "document.title"})
        assert resp.status_code == 200
        assert "Example" in str(resp.json().get("result", ""))

        # Step 5: Get page content via JavaScript
        resp = client.post(
            "/browser/evaluate",
            json={"script": "document.body.innerText.substring(0, 100)"}
        )
        assert resp.status_code == 200
        result = resp.json()
        assert "result" in result

        # Step 6: Click on a link (example.com has a "More information" link)
        resp = client.post(
            "/browser/evaluate",
            json={"script": "document.querySelector('a')?.href || 'no link'"}
        )
        assert resp.status_code == 200

        # Step 7: Close browser
        resp = client.post("/browser/close")
        assert resp.status_code == 200

    def test_file_processing_pipeline(self, client):
        """
        Multi-turn: Create files, process them, aggregate results.
        Simulates data processing workflow.
        """
        # Step 1: Create multiple data files
        for i in range(3):
            content = f"data_{i}={i * 10}\n"
            resp = client.post(
                "/file/write", json={"path": f"data_{i}.txt", "content": content}
            )
            assert resp.status_code == 200

        # Step 2: List files to verify
        resp = client.get("/file/list", params={"path": "."})
        assert resp.status_code == 200
        files = [e["name"] for e in resp.json()["entries"]]
        assert "data_0.txt" in files
        assert "data_1.txt" in files
        assert "data_2.txt" in files

        # Step 3: Process files with shell command
        resp = client.post(
            "/shell/exec", json={"command": "cat data_*.txt | sort"}
        )
        assert resp.status_code == 200
        output = resp.json()["stdout"]
        assert "data_0=0" in output
        assert "data_2=20" in output

        # Step 4: Aggregate with Python
        aggregate_script = """
import glob
total = 0
for f in glob.glob('data_*.txt'):
    with open(f) as file:
        line = file.read().strip()
        val = int(line.split('=')[1])
        total += val
print(f"Total: {total}")
"""
        resp = client.post(
            "/code/execute", json={"code": aggregate_script, "language": "python"}
        )
        assert resp.status_code == 200
        assert "Total: 30" in resp.json()["output"]

        # Step 5: Clean up
        resp = client.post("/shell/exec", json={"command": "rm data_*.txt"})
        assert resp.status_code == 200

    def test_browser_screenshot_comparison(self, client):
        """
        Multi-turn: Navigate to different pages, compare screenshots.
        Simulates visual testing workflow.
        """
        screenshots = {}

        # Step 1: Launch browser
        client.post("/browser/launch")

        # Step 2: Navigate and screenshot page 1
        client.post("/browser/navigate", json={"url": "https://example.com"})
        resp = client.get("/browser/screenshot")
        assert resp.status_code == 200
        screenshots["example"] = resp.content

        # Step 3: Navigate and screenshot page 2
        client.post("/browser/navigate", json={"url": "https://httpbin.org"})
        resp = client.get("/browser/screenshot")
        assert resp.status_code == 200
        screenshots["httpbin"] = resp.content

        # Step 4: Verify screenshots are different
        assert screenshots["example"] != screenshots["httpbin"]

        # Step 5: Verify both are valid images
        for name, data in screenshots.items():
            img = Image.open(io.BytesIO(data))
            assert img.width > 0, f"{name} screenshot invalid"

        # Step 6: Close browser
        client.post("/browser/close")

    def test_iterative_code_development(self, client):
        """
        Multi-turn: Write code, test it, fix bugs, iterate.
        Simulates iterative development workflow.
        """
        # Step 1: Write initial (buggy) code
        buggy_code = """
def add(a, b):
    return a - b  # Bug: should be +

result = add(2, 3)
print(f"Result: {result}")
"""
        resp = client.post(
            "/file/write", json={"path": "math_utils.py", "content": buggy_code}
        )
        assert resp.status_code == 200

        # Step 2: Run tests to find bug
        test_code = """
exec(open('math_utils.py').read())
assert add(2, 3) == 5, f"Expected 5, got {add(2, 3)}"
print("Tests passed!")
"""
        resp = client.post(
            "/code/execute", json={"code": test_code, "language": "python"}
        )
        data = resp.json()
        assert data["exit_code"] != 0  # Should fail

        # Step 3: Read the buggy code
        resp = client.get("/file/read", params={"path": "math_utils.py"})
        assert resp.status_code == 200
        original = resp.json()["content"]

        # Step 4: Fix the bug
        fixed_code = original.replace("a - b", "a + b")
        resp = client.post(
            "/file/write", json={"path": "math_utils.py", "content": fixed_code}
        )
        assert resp.status_code == 200

        # Step 5: Re-run tests
        resp = client.post(
            "/code/execute", json={"code": test_code, "language": "python"}
        )
        data = resp.json()
        assert data["exit_code"] == 0
        assert "Tests passed!" in data["output"]

        # Step 6: Clean up
        client.post("/shell/exec", json={"command": "rm math_utils.py"})

    def test_desktop_automation_workflow(self, client):
        """
        Multi-turn: Combine browser and desktop automation.
        Simulates hybrid automation workflow.
        """
        # Step 1: Take initial desktop screenshot
        resp = client.get("/screen/screenshot")
        assert resp.status_code == 200
        initial_screenshot = resp.content

        # Step 2: Launch browser (will appear on desktop)
        resp = client.post("/browser/launch", params={"headless": False})
        assert resp.status_code == 200

        # Step 3: Navigate to a page
        resp = client.post(
            "/browser/navigate", json={"url": "https://example.com"}
        )
        assert resp.status_code == 200

        # Step 4: Wait a moment for rendering
        time.sleep(1)

        # Step 5: Take desktop screenshot (should show browser)
        resp = client.get("/screen/screenshot")
        assert resp.status_code == 200
        with_browser_screenshot = resp.content

        # Step 6: Screenshots should be different
        assert initial_screenshot != with_browser_screenshot

        # Step 7: Close browser
        client.post("/browser/close")


# ============================================================================
# Stress Tests
# ============================================================================


class TestStress:
    """Stress and reliability tests."""

    def test_rapid_shell_commands(self, client):
        """Execute many shell commands rapidly."""
        for i in range(10):
            resp = client.post("/shell/exec", json={"command": f"echo {i}"})
            assert resp.status_code == 200
            assert str(i) in resp.json()["stdout"]

    def test_large_file_handling(self, client):
        """Handle large file content."""
        large_content = "X" * 100000  # 100KB
        resp = client.post(
            "/file/write", json={"path": "large_file.txt", "content": large_content}
        )
        assert resp.status_code == 200

        resp = client.get("/file/read", params={"path": "large_file.txt"})
        assert resp.status_code == 200
        assert len(resp.json()["content"]) == 100000

        client.post("/shell/exec", json={"command": "rm large_file.txt"})

    def test_concurrent_operations(self, client, api_url):
        """Test concurrent API calls."""
        import concurrent.futures

        def make_request(i):
            with httpx.Client(base_url=api_url, timeout=30.0) as c:
                resp = c.post("/shell/exec", json={"command": f"echo {i}"})
                return resp.status_code, i

        with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
            futures = [executor.submit(make_request, i) for i in range(10)]
            results = [f.result() for f in concurrent.futures.as_completed(futures)]

        assert all(status == 200 for status, _ in results)


# ============================================================================
# Goal-Oriented Task Tests
# ============================================================================


class TestGoalOrientedTasks:
    """
    Tests that simulate goal-oriented multi-step tasks.
    These test the API's ability to support complex automation goals.
    """

    def test_goal_setup_dev_environment(self, client):
        """
        Goal: Set up a minimal development environment.
        Steps: Create project structure, write config, verify.
        """
        # Clean up any previous runs first
        client.post("/shell/exec", json={"command": "rm -rf myproject"})

        # Create project structure
        dirs = ["myproject", "myproject/src", "myproject/tests"]
        for d in dirs:
            resp = client.post(
                "/shell/exec", json={"command": f"mkdir -p {d}"}
            )
            assert resp.status_code == 200

        # Create main source file (note: no leading newline to avoid indentation issues)
        main_py = """def main():
    print("Hello from myproject!")
    return 0

if __name__ == "__main__":
    main()
"""
        resp = client.post(
            "/file/write",
            json={"path": "myproject/src/main.py", "content": main_py},
        )
        assert resp.status_code == 200

        # Create test file that uses absolute import path
        test_py = """import sys
import os

# Add the src directory to path (relative to workspace)
workspace = os.environ.get('WORKSPACE', '/home/sandbox/workspace')
sys.path.insert(0, os.path.join(workspace, 'myproject/src'))

from main import main

def test_main():
    result = main()
    assert result == 0, f"Expected 0, got {result}"
    print("Test passed!")

test_main()
"""
        resp = client.post(
            "/file/write",
            json={"path": "myproject/tests/test_main.py", "content": test_py},
        )
        assert resp.status_code == 200

        # Run the test
        resp = client.post(
            "/shell/exec",
            json={"command": "python3 myproject/tests/test_main.py"},
        )
        assert resp.status_code == 200
        result = resp.json()
        # Check for success - either in stdout or verify no errors
        assert result["exit_code"] == 0, f"Test failed with stderr: {result['stderr']}"
        assert "Test passed!" in result["stdout"] or "Hello from myproject!" in result["stdout"]

        # Clean up
        client.post("/shell/exec", json={"command": "rm -rf myproject"})

    def test_goal_web_scraping(self, client):
        """
        Goal: Extract information from a web page.
        Steps: Navigate, extract data via JS, process results.
        """
        # Launch browser
        client.post("/browser/launch")

        # Navigate to target
        resp = client.post(
            "/browser/navigate", json={"url": "https://example.com"}
        )
        assert resp.status_code == 200

        # Extract page title
        resp = client.post(
            "/browser/evaluate", json={"script": "document.title"}
        )
        assert resp.status_code == 200
        title = resp.json()["result"]
        assert title is not None

        # Extract all links
        resp = client.post(
            "/browser/evaluate",
            json={
                "script": "Array.from(document.querySelectorAll('a')).map(a => ({href: a.href, text: a.textContent}))"
            },
        )
        assert resp.status_code == 200
        links = resp.json()["result"]
        assert isinstance(links, list)

        # Extract main heading
        resp = client.post(
            "/browser/evaluate",
            json={"script": "document.querySelector('h1')?.textContent || 'No h1 found'"},
        )
        assert resp.status_code == 200

        # Save extracted data
        import json
        extracted = {"title": title, "links": links}
        resp = client.post(
            "/file/write",
            json={"path": "extracted_data.json", "content": json.dumps(extracted, indent=2)},
        )
        assert resp.status_code == 200

        # Verify saved data
        resp = client.get("/file/read", params={"path": "extracted_data.json"})
        assert resp.status_code == 200

        # Clean up
        client.post("/browser/close")
        client.post("/shell/exec", json={"command": "rm extracted_data.json"})

    def test_goal_automated_testing(self, client):
        """
        Goal: Run an automated test suite and generate a report.
        Steps: Create tests, run them, collect results, generate report.
        """
        # Create a module to test
        module_code = """
def calculate_factorial(n):
    if n < 0:
        raise ValueError("Negative numbers not allowed")
    if n <= 1:
        return 1
    return n * calculate_factorial(n - 1)

def is_palindrome(s):
    s = s.lower().replace(" ", "")
    return s == s[::-1]
"""
        client.post(
            "/file/write", json={"path": "utils.py", "content": module_code}
        )

        # Create test suite
        test_code = """
import sys
import json

# Import module
exec(open('utils.py').read())

results = {"passed": 0, "failed": 0, "tests": []}

def run_test(name, test_fn):
    try:
        test_fn()
        results["passed"] += 1
        results["tests"].append({"name": name, "status": "passed"})
        print(f"✓ {name}")
    except Exception as e:
        results["failed"] += 1
        results["tests"].append({"name": name, "status": "failed", "error": str(e)})
        print(f"✗ {name}: {e}")

# Test cases
run_test("factorial_zero", lambda: assert_eq(calculate_factorial(0), 1))
run_test("factorial_one", lambda: assert_eq(calculate_factorial(1), 1))
run_test("factorial_five", lambda: assert_eq(calculate_factorial(5), 120))
run_test("palindrome_true", lambda: assert_eq(is_palindrome("racecar"), True))
run_test("palindrome_false", lambda: assert_eq(is_palindrome("hello"), False))
run_test("palindrome_spaces", lambda: assert_eq(is_palindrome("A man a plan a canal Panama"), True))

def assert_eq(actual, expected):
    assert actual == expected, f"Expected {expected}, got {actual}"

# Re-run with assert_eq defined
results = {"passed": 0, "failed": 0, "tests": []}
run_test("factorial_zero", lambda: assert_eq(calculate_factorial(0), 1))
run_test("factorial_one", lambda: assert_eq(calculate_factorial(1), 1))
run_test("factorial_five", lambda: assert_eq(calculate_factorial(5), 120))
run_test("palindrome_true", lambda: assert_eq(is_palindrome("racecar"), True))
run_test("palindrome_false", lambda: assert_eq(is_palindrome("hello"), False))
run_test("palindrome_spaces", lambda: assert_eq(is_palindrome("A man a plan a canal Panama"), True))

print(f"\\nResults: {results['passed']} passed, {results['failed']} failed")
print(json.dumps(results))
"""
        client.post(
            "/file/write", json={"path": "run_tests.py", "content": test_code}
        )

        # Run tests
        resp = client.post(
            "/shell/exec", json={"command": "python run_tests.py"}
        )
        assert resp.status_code == 200
        output = resp.json()["stdout"]
        assert "passed" in output

        # Clean up
        client.post("/shell/exec", json={"command": "rm utils.py run_tests.py"})


# ============================================================================
# Main
# ============================================================================


if __name__ == "__main__":
    import sys

    # Default to localhost if no URL provided
    api_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
    print(f"Running tests against: {api_url}")

    pytest.main([__file__, "-v", f"--api-url={api_url}"])
