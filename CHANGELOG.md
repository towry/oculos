# Changelog

All notable changes to OculOS will be documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/).

## [0.1.0] — 2026-03-08

### Added
- **REST API** — full UI automation over HTTP (discovery, interactions, window ops)
- **MCP server** — `--mcp` flag for AI agent integration (Claude, Cursor, Windsurf…)
- **Web dashboard** — element tree inspector, recorder, live WebSocket events
- **Cross-platform** — Windows (UI Automation), Linux (AT-SPI2), macOS (Accessibility API)
- **Element interactions** — click, set-text, send-keys, toggle, expand, collapse, select, set-range, scroll, highlight
- **Wait/poll endpoint** — `GET /windows/{pid}/wait` with configurable timeout
- **Screenshot capture** — `GET /windows/{pid}/screenshot` returns PNG (Windows)
- **Batch operations** — `POST /interact/batch` for multiple actions in one request
- **Smart error codes** — 404 for not found, 400 for invalid, 500 for server errors
- **Python SDK** — `sdk/python/` with full API wrapper
- **TypeScript SDK** — `sdk/typescript/` with typed async client
- **GitHub Actions CI** — build + lint on Windows, Linux, macOS
- **Release workflow** — auto-build binaries on tag push
- **Examples** — 7 ready-to-run scripts (Python + curl)
- **OpenAPI spec** — `openapi.yaml` for API documentation
- **Docker support** — `Dockerfile` for Linux builds
