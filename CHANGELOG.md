# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-02

### Added

- 21 MCP tools across 9 groups (properties, analytics, discovery, appearance, export, inspection, sitemaps, indexing, meta)
- `keyword_opportunities` with 5 analysis modes: `quick_wins`, `cannibalization`, `ctr_gaps`, `declining`, `growing`
- `search_analytics` breakdown param for device/country comparison splits
- OAuth 2.0 with interactive browser flow and automatic token refresh
- Service Account auth with in-memory JWT RS256 signing (`ring`)
- Error-as-UI pattern: tool errors return guidance text, never propagate to MCP layer
- Automatic retry on 429/5xx with exponential backoff, single 401 token refresh
- CSV export with formula injection protection (`=`, `+`, `-`, `@`, `\t` prefix escaping)
- Client-side sorting and pagination for all analytics queries
- Month-aware date validation including leap years
- Docker multi-stage build
- stdio transport for Claude Desktop and Claude Code
- Rust edition 2024, clippy pedantic lints enabled

[Unreleased]: https://github.com/eyupcanakman/gsc-mcp-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/eyupcanakman/gsc-mcp-rs/releases/tag/v0.1.0
