"""
ComfyUI Desktop Logs — Backend log viewer extension.

Serves the Rust launcher's log file via HTTP so the frontend floating panel
can display ROCm, Python, and backend logs in real-time.

Endpoints:
  GET /logs/latest          → last N lines (default 200)
  GET /logs/stream?after=id → long-poll for new lines after given id
  GET /logs/stats           → file size + line count
"""

import asyncio
import os
import json
import time
import threading
from pathlib import Path
from aiohttp import web

import server

LOG_FILE_CANDIDATES = [
    Path.home() / ".cache" / "comfyui-desktop" / "comfyui-backend.log",
    Path("/tmp") / "comfyui-backend.log",
]

POLL_INTERVAL = 0.5  # seconds


def _find_log_file() -> Path | None:
    for p in LOG_FILE_CANDIDATES:
        if p.exists():
            return p
    return None


def _read_tail(path: Path, n: int = 200) -> list[dict]:
    """Read last n lines from log file, return as list of {id, text}."""
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
    except Exception:
        return []

    tail = lines[-n:] if len(lines) > n else lines
    result = []
    for i, line in enumerate(tail):
        global_offset = max(0, len(lines) - len(tail)) + i
        result.append({
            "id": global_offset,
            "text": line.rstrip("\n"),
        })
    return result


def _read_after(path: Path, after_id: int) -> list[dict]:
    """Read lines with id > after_id."""
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
    except Exception:
        return []

    result = []
    for i in range(after_id + 1, len(lines)):
        result.append({
            "id": i,
            "text": lines[i].rstrip("\n"),
        })
    return result


def _get_stats(path: Path) -> dict:
    try:
        stat = path.stat()
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            line_count = sum(1 for _ in f)
        return {
            "size_bytes": stat.st_size,
            "line_count": line_count,
            "path": str(path),
        }
    except Exception:
        return {"size_bytes": 0, "line_count": 0, "path": ""}


async def handle_latest(request):
    """GET /logs/latest?lines=200"""
    n = int(request.query.get("lines", 200))
    log_file = _find_log_file()
    if log_file is None:
        return web.json_response({"lines": [], "error": "Log file not found"})
    lines = _read_tail(log_file, n)
    return web.json_response({"lines": lines})


async def handle_stream(request):
    """GET /logs/stream?after=0 — long-poll until new lines appear or timeout."""
    after_id = int(request.query.get("after", 0))
    log_file = _find_log_file()
    if log_file is None:
        return web.json_response({"lines": [], "error": "Log file not found"})

    # Poll for new lines (max 30s timeout)
    deadline = time.time() + 30
    while time.time() < deadline:
        lines = _read_after(log_file, after_id)
        if lines:
            return web.json_response({"lines": lines})
        await asyncio.sleep(POLL_INTERVAL)

    return web.json_response({"lines": []})


async def handle_stats(request):
    """GET /logs/stats"""
    log_file = _find_log_file()
    if log_file is None:
        return web.json_response({"error": "Log file not found"})
    return web.json_response(_get_stats(log_file))


# Register routes
routes = [
    web.get("/logs/latest", handle_latest),
    web.get("/logs/stream", handle_stream),
    web.get("/logs/stats", handle_stats),
]

server.app.add_routes(routes)
print("[ComfyUI-Desktop-Logs] Registered /logs/* endpoints")
