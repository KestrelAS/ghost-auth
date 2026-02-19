# Security Policy

Ghost Auth is a security-focused application that handles TOTP secrets. We take vulnerability reports seriously.

## Reporting a Vulnerability

**Do not open a public issue for security vulnerabilities.**

Instead, use [GitHub's private vulnerability reporting](https://github.com/KestrelAS/ghost-auth/security/advisories/new) to submit your report. This ensures the details remain confidential until a fix is available.

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Affected components (e.g., `storage.rs`, `keystore.rs`, `backup.rs`)
- Potential impact
- Suggested fix (if you have one)

### What Counts as a Security Issue

- Bypass of PIN lock or rate limiting
- Exposure of TOTP secrets to the frontend or logs
- Weakness in encryption (AES-256-GCM, Argon2 parameters)
- Keychain/keystore integration flaws
- Backup format vulnerabilities (key derivation, nonce reuse)
- CSP bypass or code injection

### What to File as a Regular Bug Instead

- UI glitches or cosmetic issues
- Build or compilation errors
- Feature requests
- Performance issues (unless they enable a timing attack)

## Response Timeline

- **Acknowledgment:** Within 48 hours
- **Initial assessment:** Within 1 week
- **Fix or mitigation:** Depends on severity, but we aim for critical issues within 2 weeks

## Credit

If you report a valid vulnerability, we're happy to credit you in the release notes (unless you prefer to remain anonymous).

## Security Design Overview

For an overview of Ghost Auth's security architecture, see the [Security Model](README.md#security-model) section in the README.
