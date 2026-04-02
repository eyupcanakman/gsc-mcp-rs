# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

If you discover a security vulnerability in gsc-mcp-rs, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please open a private security advisory via GitHub's **"Report a vulnerability"** button on this repository.

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 1 week
- **Fix or mitigation**: Targeting 30 days for critical issues

### Disclosure Policy

We follow coordinated disclosure. Once a fix is released, we will:
1. Publish a security advisory on GitHub
2. Credit the reporter (unless they prefer to remain anonymous)
3. Release a patched version

## Security Design

gsc-mcp-rs handles Google OAuth tokens and service account keys. Key security measures:

- Tokens stored with 0600 permissions (owner-only read/write), config directory 0700
- Credentials never logged. OAuthToken has redacted Debug impl
- Token URI validated. Service account keys must use exact `https://oauth2.googleapis.com/token`
- In-memory JWT signing. Private keys never written to disk; `ring` handles RSA-PKCS1-SHA256
- Error responses sanitized. API error bodies are filtered before display
- CSV formula injection protection. Exported values starting with `=`, `+`, `-`, `@`, `\t` are escaped
- stdout is reserved for JSON-RPC protocol data only; no credentials leak to stdout
