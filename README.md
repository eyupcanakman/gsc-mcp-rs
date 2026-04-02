# gsc-mcp-rs

[![CI](https://github.com/eyupcanakman/gsc-mcp-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/eyupcanakman/gsc-mcp-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust: 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)

A fast, single-binary MCP server for Google Search Console. 21 tools covering search analytics, URL inspection, sitemaps, indexing, and property management.

## Features

- 21 tools across 9 groups: analytics, discovery, inspection, sitemaps, indexing, properties, meta
- Both auth methods: OAuth 2.0 (personal) and Service Account (automation)
- Error-as-UI: operational errors return guidance text, never crash the conversation
- Zero runtime deps: single Rust binary, no Node/Python/Java required
- sc-domain: support: domain properties work correctly from day one

## Quick Start

### Install

```bash
cargo install gsc-mcp-rs
```

Or build from source:

```bash
git clone https://github.com/eyupcanakman/gsc-mcp-rs.git
cd gsc-mcp-rs
cargo build --release
```

### Configure Auth

**Option 1: OAuth (personal use)**

1. Go to [Google Cloud Console](https://console.cloud.google.com/) > APIs & Services > Credentials
2. Create an OAuth 2.0 Client ID (Desktop app type)
3. Enable the Search Console API and Indexing API
4. Save credentials:

```bash
mkdir -p ~/.config/gsc-mcp-rs
cat > ~/.config/gsc-mcp-rs/oauth_credentials.json << 'EOF'
{"client_id": "YOUR_CLIENT_ID", "client_secret": "YOUR_CLIENT_SECRET"}
EOF
```

5. Authenticate (opens browser, completes via localhost callback):

```bash
gsc-mcp-rs auth
```

**Option 2: Service Account (automation)**

1. Create a service account in Google Cloud Console
2. Grant it access to your Search Console properties (Full permission)
3. Download the JSON key and place it:

```bash
cp your-key.json ~/.config/gsc-mcp-rs/service_account.json
```

Or set the environment variable:

```bash
export GSC_SERVICE_ACCOUNT_PATH=/path/to/key.json
```

### Add to Claude Desktop

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "gsc": {
      "command": "gsc-mcp-rs",
      "args": ["stdio"]
    }
  }
}
```

### Add to Claude Code

```bash
claude mcp add gsc-mcp-rs -- gsc-mcp-rs stdio
```

## Tools (21)

### Properties & Auth
| Tool | Description |
|------|-------------|
| `list_sites` | List all accessible GSC properties with permission levels |
| `get_site_details` | Get verification status and ownership details |
| `manage_site` | Add or remove a property (`action: "add"\|"delete"`, idempotent) |
| `reauthenticate` | Clear tokens for account switching |

### Search Analytics
| Tool | Description |
|------|-------------|
| `search_analytics` | All dimensions, filters, 25K rows, sorting. Use `breakdown` for device/country splits |
| `compare_periods` | Side-by-side period comparison with deltas |
| `performance_overview` | Quick aggregate summary + daily trend |
| `page_query_breakdown` | Which queries drive traffic to a specific page |

### Discovery & Analysis
| Tool | Description |
|------|-------------|
| `top_pages` | Best-performing pages by clicks/impressions/CTR/position |
| `keyword_opportunities` | 5 modes: `quick_wins` (default), `cannibalization`, `ctr_gaps`, `declining`, `growing` |
| `brand_query_analysis` | Brand vs non-brand traffic split |
| `keyword_trend` | Daily tracking of a single keyword |
| `detect_anomalies` | Automatic drop detection with severity levels |

### Search Appearance
| Tool | Description |
|------|-------------|
| `query_by_search_appearance` | Filter by rich result type (FAQ, VIDEO, etc.) |

### Export
| Tool | Description |
|------|-------------|
| `export_analytics` | Force CSV file export for large datasets |

### URL Inspection
| Tool | Description |
|------|-------------|
| `inspect_url` | Check indexing status of a single URL |
| `batch_inspect_urls` | Inspect up to 50 URLs with configurable concurrency |

### Sitemaps
| Tool | Description |
|------|-------------|
| `list_sitemaps` | List submitted sitemaps with status |
| `manage_sitemap` | Submit or remove a sitemap (`action: "submit"\|"delete"`, idempotent) |

### Indexing API
| Tool | Description |
|------|-------------|
| `request_indexing` | Request Google to crawl a URL |

### Meta
| Tool | Description |
|------|-------------|
| `api_reference` | All dimensions and metrics with descriptions and value ranges |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GSC_MCP_CONFIG_DIR` | `~/.config/gsc-mcp-rs/` | Config and token storage directory |
| `GSC_MCP_OUTPUT_DIR` | System temp dir | CSV export directory |
| `GSC_SERVICE_ACCOUNT_PATH` | (none) | Path to service account JSON key |

## CLI Usage

```bash
gsc-mcp-rs stdio         # stdio transport (default)
gsc-mcp-rs auth          # Interactive OAuth flow
gsc-mcp-rs --version     # Show version
gsc-mcp-rs --help        # Show help
```

## Docker

```bash
docker build -t gsc-mcp-rs .
docker run -i --rm -v ~/.config/gsc-mcp-rs:/root/.config/gsc-mcp-rs gsc-mcp-rs
```

## Minimum Supported Rust Version (MSRV)

Rust **1.85** or later.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## Security

See [SECURITY.md](SECURITY.md) for the vulnerability reporting process.

## License

MIT. See [LICENSE](LICENSE).
