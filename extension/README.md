# Ghost Auth — Browser Extension

A companion browser extension for [Ghost Auth](../README.md) that brings TOTP code generation to Chrome and Firefox. Syncs accounts with the desktop/mobile app over LAN.

## Building

```bash
npm install

# Chrome (Manifest V3)
npm run build:chrome

# Firefox (Manifest V3)
npm run build:firefox

# Both
npm run build:all
```

Built extension will be in `dist/chrome/` or `dist/firefox/`.

## Testing

```bash
npm test
```

## Security Model

The browser extension operates under a **different security model** than the desktop/mobile app. Users should understand these differences:

### Desktop/mobile app (stronger)

- TOTP secrets are stored and processed exclusively in Rust
- The frontend (WebView) never receives raw secrets — only generated 6/8-digit codes
- Encryption keys are stored in the OS keychain (macOS Keychain, iOS Keychain, Linux Secret Service)
- A compromised WebView cannot extract TOTP secrets

### Browser extension (weaker by necessity)

- TOTP secrets must be handled in JavaScript — browser extensions cannot use native OS keystores or run Rust code in-process
- Secrets are encrypted at rest with AES-256-GCM; the encryption key (DEK) is derived from a master password via Argon2id (64 MB, 3 iterations)
- The DEK is held in memory while the extension is unlocked, and cached in `chrome.storage.session` (memory-only, cleared on browser restart) to survive popup close/reopen
- **Passwordless mode** stores the DEK in `chrome.storage.local` without encryption for convenience. The extension shows a clear warning before enabling this: anyone with access to the browser profile can read all secrets. This mode is opt-in and disabled by default

### Shared security properties

- PIN lock with Argon2id hashing and escalating rate limiting (30s → 5min → 15min)
- 8 individually-hashed recovery codes per PIN
- Encrypted backups with Argon2id key derivation (GHST format, compatible with the desktop app)
- Device-to-device sync with HMAC-SHA256 mutual authentication and per-account AES-256-GCM encryption
