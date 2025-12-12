"""Pytest configuration for sandbox API tests."""

import os
import time
from datetime import datetime
from pathlib import Path

import pytest


def pytest_addoption(parser):
    """Add custom command line options."""
    parser.addoption(
        "--api-url",
        action="store",
        default=os.getenv("SANDBOX_API_URL", "http://localhost:8080"),
        help="Sandbox API URL (default: http://localhost:8080 or SANDBOX_API_URL env var)",
    )
    parser.addoption(
        "--record",
        action="store_true",
        default=False,
        help="Record screen during tests (saves to ./recordings/)",
    )
    parser.addoption(
        "--record-dir",
        action="store",
        default=os.getenv("SANDBOX_RECORD_DIR", "./recordings"),
        help="Directory to save recordings (default: ./recordings)",
    )
    parser.addoption(
        "--record-fps",
        action="store",
        type=int,
        default=15,
        help="Recording framerate (default: 15)",
    )


def pytest_configure(config):
    """Configure custom markers."""
    config.addinivalue_line(
        "markers", "slow: marks tests as slow (deselect with '-m \"not slow\"')"
    )
    config.addinivalue_line(
        "markers", "browser: marks tests that require browser"
    )
    config.addinivalue_line(
        "markers", "desktop: marks tests that require desktop/X11"
    )


@pytest.fixture(scope="session")
def api_url(request):
    """Get the API URL from command line or environment."""
    return request.config.getoption("--api-url")


@pytest.fixture(scope="session")
def record_enabled(request):
    """Check if recording is enabled."""
    return request.config.getoption("--record")


@pytest.fixture(scope="session")
def record_dir(request):
    """Get the recording directory."""
    return request.config.getoption("--record-dir")


@pytest.fixture(scope="session")
def record_fps(request):
    """Get recording framerate."""
    return request.config.getoption("--record-fps")


@pytest.fixture(scope="session")
def base_client(api_url):
    """Create a session-scoped HTTP client."""
    import httpx
    with httpx.Client(base_url=api_url, timeout=60.0) as client:
        yield client


@pytest.fixture
def client(api_url):
    """Create a function-scoped HTTP client."""
    import httpx
    with httpx.Client(base_url=api_url, timeout=60.0) as client:
        yield client


@pytest.fixture
async def async_client(api_url):
    """Create an async HTTP client."""
    import httpx
    async with httpx.AsyncClient(base_url=api_url, timeout=60.0) as client:
        yield client


@pytest.fixture(autouse=True)
def auto_record(request, api_url, record_enabled, record_dir, record_fps):
    """
    Automatically record screen during each test when --record is enabled.

    Recordings are saved to: {record_dir}/{test_class}/{test_name}_{timestamp}.mp4
    """
    if not record_enabled:
        yield
        return

    import httpx

    # Create recording directory
    test_name = request.node.name
    test_class = request.node.parent.name if request.node.parent else "unknown"

    # Sanitize names for filesystem
    test_class = test_class.replace("::", "_").replace("/", "_")
    test_name = test_name.replace("::", "_").replace("/", "_").replace("[", "_").replace("]", "_")

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

    local_dir = Path(record_dir) / test_class
    local_dir.mkdir(parents=True, exist_ok=True)

    # Path on the sandbox (remote)
    remote_path = f"/tmp/recordings/{test_class}_{test_name}_{timestamp}.mp4"
    local_path = local_dir / f"{test_name}_{timestamp}.mp4"

    client = httpx.Client(base_url=api_url, timeout=120.0)
    recording_started = False

    try:
        # Start recording
        resp = client.post(
            "/screen/record/start",
            json={"output_path": remote_path, "fps": record_fps}
        )
        if resp.status_code == 200:
            recording_started = True
            print(f"\n  Recording started: {remote_path}")
            # Small delay to ensure recording has started
            time.sleep(1.0)
        else:
            print(f"\n  Warning: Could not start recording: {resp.status_code} - {resp.text}")
    except Exception as e:
        print(f"\n  Warning: Could not start recording: {e}")

    yield

    if recording_started:
        try:
            # Stop recording - give it time to write final frames
            time.sleep(1.0)
            resp = client.post("/screen/record/stop")

            if resp.status_code == 200:
                result = resp.json()
                print(f"\n  Recording stopped: {result}")

                # Wait a moment for file to be finalized
                time.sleep(0.5)

                # Download the recording
                if result.get("size_bytes", 0) > 0:
                    download_resp = client.get(
                        "/screen/record/download",
                        params={"path": remote_path},
                        timeout=120.0
                    )
                    if download_resp.status_code == 200:
                        with open(local_path, "wb") as f:
                            f.write(download_resp.content)
                        print(f"  Recording saved: {local_path} ({len(download_resp.content)} bytes)")
                    else:
                        print(f"\n  Warning: Could not download recording: {download_resp.status_code}")
                elif result.get("error"):
                    print(f"\n  Warning: Recording error: {result.get('error')}")
                else:
                    print(f"\n  Warning: Recording file is empty or not created")
            else:
                print(f"\n  Warning: Could not stop recording: {resp.status_code} - {resp.text}")
        except Exception as e:
            print(f"\n  Warning: Could not stop/download recording: {e}")
        finally:
            client.close()


@pytest.fixture
def record_test(api_url, record_dir):
    """
    Manual recording fixture for tests that want explicit control.

    Usage:
        def test_something(record_test):
            with record_test("my_custom_name"):
                # ... test code that will be recorded ...
    """
    import httpx
    from contextlib import contextmanager

    @contextmanager
    def _record(name: str, fps: int = 15):
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        remote_path = f"/tmp/recordings/{name}_{timestamp}.mp4"
        local_dir = Path(record_dir)
        local_dir.mkdir(parents=True, exist_ok=True)
        local_path = local_dir / f"{name}_{timestamp}.mp4"

        client = httpx.Client(base_url=api_url, timeout=60.0)

        try:
            # Start
            resp = client.post(
                "/screen/record/start",
                json={"output_path": remote_path, "fps": fps}
            )
            started = resp.status_code == 200
            time.sleep(0.5)

            yield local_path

            # Stop and download
            if started:
                time.sleep(0.5)
                client.post("/screen/record/stop")
                time.sleep(0.5)

                download_resp = client.get(
                    "/screen/record/download",
                    params={"path": remote_path}
                )
                if download_resp.status_code == 200:
                    with open(local_path, "wb") as f:
                        f.write(download_resp.content)
                    print(f"\n  Recording saved: {local_path}")
        finally:
            client.close()

    return _record
