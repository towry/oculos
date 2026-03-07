# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅        |

## Reporting a Vulnerability

If you discover a security vulnerability in OculOS, please report it responsibly:

1. **Do NOT open a public issue.**
2. Email **huseyinstif@gmail.com** with:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
3. You will receive a response within 48 hours.

## Security Model

OculOS binds to `127.0.0.1` by default — it is **not** accessible from the network. If you bind to `0.0.0.0`, you are exposing full desktop control to your network. Do not do this without additional authentication.

**OculOS does not include:**
- Authentication or API keys (localhost-only by design)
- Encryption (HTTP, not HTTPS)
- Rate limiting
- Sandboxing of interactions

**If you need remote access**, use an SSH tunnel or VPN:

```bash
ssh -L 7878:127.0.0.1:7878 user@remote-machine
```

## Scope

The following are considered security issues:
- Remote code execution without user interaction
- Privilege escalation through the API
- Unintended network exposure
- Data exfiltration through the accessibility tree

The following are **not** security issues:
- An authenticated local user controlling desktop apps (this is the intended behavior)
- Accessibility tree exposing UI content (this is how OS accessibility works)
