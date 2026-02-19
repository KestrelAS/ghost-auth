# Contributing to Ghost Auth

Thanks for your interest in contributing to Ghost Auth! This document covers everything you need to get started.

## Development Setup

### Prerequisites

- [Node.js](https://nodejs.org/) v18+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri CLI prerequisites](https://tauri.app/start/prerequisites/) for your platform

### Getting Started

```bash
git clone https://github.com/KestrelAS/ghost-auth.git
cd ghost-auth
npm install
npm run tauri dev
```

## Project Structure

The app is split into two layers:

- **`src/`** — Svelte 5 frontend (UI components, stores, styles)
- **`src-tauri/src/`** — Rust backend (cryptography, storage, TOTP generation)

All security-critical code lives in the Rust backend. The frontend never handles raw TOTP secrets — it only receives generated codes via Tauri commands.

See the [README](README.md) for a full directory breakdown.

## Running Tests

```bash
# Frontend unit tests
npm test

# Frontend tests in watch mode
npm run test:watch

# TypeScript type checking
npm run check

# Rust tests
cd src-tauri && cargo test

# E2E tests (requires the dev server)
npm run test:e2e
```

## Code Style

### Rust

- Format with `cargo fmt`
- Lint with `cargo clippy --all-targets -- -D warnings` (zero warnings policy)
- All new Tauri commands need input validation in `commands.rs`

### TypeScript / Svelte

- TypeScript strict mode is enabled — no `any` types
- Unused variables and parameters are compile errors
- Run `npm run check` before submitting

## Contributor License Agreement

All contributors must sign our [Contributor License Agreement (CLA)](CLA.md) before their pull request can be merged. This assigns copyright to Kestrel AS so the project can be maintained, protected, and licensed consistently.

When you open your first PR, the CLA Assistant bot will comment with instructions — just reply with the signing phrase and you're done. It's a one-time step.

## Submitting Changes

1. Fork the repository
2. Create a feature branch from `main` (`git checkout -b my-feature`)
3. Make your changes
4. Ensure all tests pass (`npm test && cd src-tauri && cargo test`)
5. Ensure code is formatted (`cargo fmt` and no TypeScript errors)
6. Commit with a clear message describing what changed and why
7. Push to your fork and open a pull request

### Commit Messages

Write commit messages that explain the *why*, not just the *what*:

- `add backup password strength indicator` (good)
- `update BackupExport.svelte` (too vague)
- `fix rate limit bypass when system clock moves backward` (good)

### Pull Request Guidelines

- Keep PRs focused — one feature or fix per PR
- Include a description of what changed and how to test it
- If your change touches security-sensitive code (anything in `src-tauri/src/`), call that out explicitly in the PR description

## Security-Sensitive Code

The following files handle cryptographic operations and require extra care:

| File | Responsibility |
|------|---------------|
| `src-tauri/src/storage.rs` | AES-256-GCM encryption of TOTP secrets |
| `src-tauri/src/keystore.rs` | OS keychain integration (all platforms) |
| `src-tauri/src/pin.rs` | Argon2 PIN hashing and rate limiting |
| `src-tauri/src/backup.rs` | Encrypted backup format |
| `src-tauri/src/totp.rs` | TOTP code generation |

When modifying these files:

- Do not weaken encryption parameters or remove validation
- Ensure secrets are zeroized after use (`zeroize` crate)
- Never expose raw TOTP secrets to the frontend
- Add tests for any new code paths

## Reporting Security Issues

If you find a security vulnerability, **do not open a public issue**. See [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

## License

By contributing, you agree to the terms of our [Contributor License Agreement](CLA.md). All contributions are licensed under the [GNU General Public License v3](LICENSE).
