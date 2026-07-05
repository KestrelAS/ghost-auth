# Ghost Auth

[![CI](https://github.com/KestrelAS/ghost-auth/actions/workflows/ci.yml/badge.svg)](https://github.com/KestrelAS/ghost-auth/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

![Windows](https://img.shields.io/badge/Windows-0078D4?style=flat&logo=windows&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat&logo=linux&logoColor=black)
![macOS](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white)
![iOS](https://img.shields.io/badge/iOS-000000?style=flat&logo=apple&logoColor=white)
![Android](https://img.shields.io/badge/Android-3DDC84?style=flat&logo=android&logoColor=white)
![Chrome](https://img.shields.io/badge/Chrome-4285F4?style=flat&logo=googlechrome&logoColor=white)
![Firefox](https://img.shields.io/badge/Firefox-FF7139?style=flat&logo=firefoxbrowser&logoColor=white)
![Edge](https://img.shields.io/badge/Edge-0078D7?style=flat&logo=microsoftedge&logoColor=white)

A cross-platform TOTP authenticator app built with [Tauri 2](https://tauri.app/), [Svelte 5](https://svelte.dev/), and Rust. Runs on Windows, macOS, Linux, iOS, Android, and as a browser extension for Chrome, Firefox, and Edge.

## Features

- **TOTP codes**: RFC 6238 compliant (SHA1, SHA256, SHA512), configurable digits and period
- **QR code scanning**: add accounts by scanning QR codes (mobile) or pasting `otpauth://` URIs
- **Manual entry**: add accounts with custom issuer, label, secret, algorithm, digits, and period
- **Import from other apps**: import accounts from Aegis, 2FAS, andOTP, FreeOTP+, Ente Auth, Bitwarden, Proton, Google Authenticator, or any file containing `otpauth://` URIs (JSON, CSV, XML, plain text)
- **Encrypted storage**: AES-256-GCM encryption with keys stored in the OS keychain (Windows Credential Manager, macOS Keychain, iOS Keychain, Android KeyStore)
- **PIN lock**: optional Argon2-hashed PIN with escalating rate limiting
- **Biometric unlock**: fingerprint/face unlock on supported devices
- **Device-to-device sync**: pair devices via QR code, sync accounts over LAN with per-account E2E encryption and conflict resolution
- **Browser extension sync**: sync accounts between the app and the companion browser extension over local network
- **iCloud sync**: automatic cloud sync across Apple devices (iOS/macOS)
- **Encrypted backups**: export/import with Argon2id key derivation and AES-GCM encryption
- **Multi-language**: 79 languages with community-contributed translations
- **Crash reporting**: opt-in, privacy-preserving error reporting with data sanitization (disabled by default)
- **Search & filter**: quickly find accounts by issuer or label
- **Account reordering**: arrange accounts in your preferred order
- **Copy to clipboard**: tap any code to copy it
- **Browser extension**: companion extension for Chrome, Firefox, and Edge with its own encrypted vault, PIN lock, and device sync

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (1.85+ stable, edition 2024)
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
├── src/                        # Svelte frontend
│   ├── App.svelte              # Main app shell
│   ├── app.css                 # Tailwind CSS theme tokens
│   └── lib/
│       ├── components/         # UI components (modals, screens, cards)
│       ├── stores/             # Reactive state (accounts, auth, theme, locale)
│       ├── utils/              # Helpers (QR scanning, otpauth parsing, error handling)
│       ├── i18n/               # Internationalization (79 locales)
│       └── assets/             # Icons and images
├── shared/                     # Code shared between app and browser extension
│   ├── components/             # Shared UI (About, Modal, EditAccount, PIN, Toast)
│   ├── stores/                 # Shared stores (toast)
│   └── utils/                  # Shared helpers (otpauth, error, focus trap)
├── src-tauri/                  # Rust backend
│   └── src/
│       ├── lib.rs              # Tauri plugin registration and app setup
│       ├── commands.rs         # Tauri command handlers
│       ├── totp.rs             # TOTP generation (RFC 6238)
│       ├── storage.rs          # Encrypted account storage (AES-256-GCM)
│       ├── keystore.rs         # OS keychain integration (Windows, macOS, iOS, Android)
│       ├── pin.rs              # PIN hashing (Argon2) and rate limiting
│       ├── backup.rs           # Encrypted backup format
│       ├── import.rs           # External app import parsing
│       ├── sync.rs             # E2E sync protocol and merge logic
│       ├── sync_transport.rs   # LAN direct sync (TCP)
│       ├── sync_ws.rs          # WebSocket sync transport (browser extension)
│       ├── icloud.rs           # iCloud sync (iOS/macOS)
│       └── crash_reporter.rs   # Privacy-preserving crash reporting
├── extension/                  # Browser extension (Chrome, Firefox, Edge)
│   ├── src/
│   │   ├── popup/              # Extension popup UI (Svelte)
│   │   ├── background/         # Service worker (auto-lock, QR scanning)
│   │   ├── content/            # Content script (QR region capture)
│   │   ├── core/               # Business logic (storage, crypto, sync, PIN)
│   │   └── lib/                # Extension-specific components and stores
│   └── manifests/              # Browser-specific manifest files
├── patches/                    # Local Tauri plugin patches
├── docs/                       # Additional documentation
├── e2e/                        # Playwright E2E tests
└── package.json
```

## Security Model

- TOTP secrets are encrypted at rest with AES-256-GCM
- Encryption keys are stored in the OS keychain, never on disk
  - **Windows:** Credential Manager
  - **macOS:** Keychain
  - **iOS:** Keychain via Security.framework
  - **Android:** Hardware-backed KeyStore (TEE/StrongBox) with AES-256-GCM key wrapping
- Secrets stay in the Rust backend; explicit export/sync actions can transfer them with user consent
- PIN is hashed with Argon2; failed attempts trigger escalating lockouts (30s, 5min, 15min) persisted across restarts
- Backups use Argon2id key derivation with a random salt before AES-GCM encryption
- Device sync uses ephemeral AES-256-GCM session keys with mutual HMAC authentication; per-account encryption ensures secrets are never sent in plaintext
- Crash reporting is disabled by default, opt-in only, and sanitizes all data through an allowlist/denylist before transmission
- CSP headers restrict script and resource loading

> **Browser extension:** The companion browser extension operates under a different security boundary: TOTP secrets must be handled in JavaScript since browser extensions cannot use native OS keystores. See [extension/README.md](extension/README.md) for the extension's security model.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, code style, and PR guidelines.

## Security

If you discover a security vulnerability, **do not open a public issue**. See [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

## License

[GPL v3](LICENSE)
