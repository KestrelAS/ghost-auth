# Ghost Auth

[![CI](https://github.com/KestrelAS/ghost-auth/actions/workflows/ci.yml/badge.svg)](https://github.com/KestrelAS/ghost-auth/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

A cross-platform TOTP authenticator app built with [Tauri 2](https://tauri.app/), [Svelte 5](https://svelte.dev/), and Rust. Strict white-on-black aesthetic, modernized and retrofied with monospace fonts and a design that hints at classic CLI tools.

## Features

- **TOTP codes** — RFC 6238 compliant (SHA1, SHA256, SHA512), configurable digits and period
- **QR code scanning** — add accounts by scanning QR codes (mobile) or pasting `otpauth://` URIs
- **Manual entry** — add accounts with custom issuer, label, secret, algorithm, digits, and period
- **Encrypted storage** — AES-256-GCM encryption with keys stored in the OS keychain (Windows Credential Manager, macOS Keychain, iOS Keychain)
- **PIN lock** — optional Argon2-hashed PIN with escalating rate limiting
- **Biometric unlock** — fingerprint/face unlock on supported devices
- **Device-to-device sync** — pair devices via QR code, sync accounts over LAN with per-account E2E encryption and conflict resolution
- **Encrypted backups** — export/import with Argon2id key derivation and AES-GCM encryption
- **Search & filter** — quickly find accounts by issuer or label
- **Account reordering** — arrange accounts in your preferred order
- **Copy to clipboard** — tap any code to copy it

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri CLI prerequisites](https://tauri.app/start/prerequisites/) for your platform

## Setup

```bash
npm install
```

## Development

```bash
# Start the Tauri dev server with hot reload
npm run tauri dev
```

## Building

```bash
# Build for production
npm run tauri build
```

Binaries will be in `src-tauri/target/release/bundle/`.

## Testing

```bash
# Frontend unit tests
npm test

# Frontend tests with watch mode
npm run test:watch

# Rust tests
cd src-tauri && cargo test

# TypeScript type checking
npm run check

# E2E tests (requires a built app)
npm run test:e2e
```

## Project Structure

```
ghost-auth/
├── src/                    # Svelte frontend
│   ├── App.svelte          # Main app shell
│   ├── lib/
│   │   ├── components/     # UI components
│   │   ├── stores/         # Tauri invoke wrappers
│   │   ├── utils/          # otpauth URI parser
│   │   └── assets/         # Static assets
│   └── app.css             # Tailwind CSS styles
├── src-tauri/              # Rust backend
│   └── src/
│       ├── lib.rs          # Tauri plugin registration
│       ├── commands.rs     # Tauri command handlers
│       ├── totp.rs         # TOTP generation (RFC 6238)
│       ├── storage.rs      # Encrypted account storage
│       ├── keystore.rs     # OS keychain integration
│       ├── pin.rs          # PIN hashing & rate limiting
│       ├── backup.rs       # Encrypted backup format
│       ├── sync.rs         # E2E sync protocol & merge logic
│       └── sync_transport.rs # LAN direct sync (TCP)
├── e2e/                    # Playwright E2E tests
└── package.json
```

## Security Model

- TOTP secrets are encrypted at rest with AES-256-GCM
- Encryption keys are stored in the OS keychain (Windows/macOS/iOS), not on disk
- Secrets never leave the Rust backend — the frontend only receives generated codes
- PIN is hashed with Argon2; failed attempts trigger escalating lockouts (30s, 5min, 15min) persisted across restarts
- Backups use Argon2id key derivation with a random salt before AES-GCM encryption
- Device sync uses ephemeral AES-256-GCM session keys with mutual HMAC authentication; per-account encryption ensures secrets are never sent in plaintext
- CSP headers restrict script and resource loading

> **Browser extension:** The companion browser extension operates under a different security boundary — TOTP secrets must be handled in JavaScript since browser extensions cannot use native OS keystores. See [extension/README.md](extension/README.md) for the extension's security model.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, code style, and PR guidelines.

## Security

If you discover a security vulnerability, **do not open a public issue**. See [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

## License

[GPL v3](LICENSE)
