# Contributing to OculOS

Thanks for your interest in OculOS! Here's how to get started.

## Development Setup

```bash
git clone https://github.com/huseyinstif/oculos.git
cd oculos
cargo build --release
```

## Making Changes

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Run `cargo build --release` to verify it compiles
4. Open a pull request

## Areas We Need Help

- **macOS backend** — `AXUIElement` accessibility API implementation
- **Client SDKs** — Python and TypeScript libraries for the REST API
- **Tests** — integration tests across different app types (Win32, Electron, Qt)
- **Dashboard** — UI improvements, new features
- **Documentation** — guides, examples, tutorials

## Code Style

- Follow existing Rust conventions in the codebase
- Keep functions focused and small
- Add comments for non-obvious logic
- No `unsafe` unless absolutely necessary (and document why)

## Reporting Bugs

Open an issue with:
- OS and version
- Steps to reproduce
- Expected vs actual behavior
- Target application (if relevant)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
