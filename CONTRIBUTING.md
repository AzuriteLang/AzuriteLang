# Contributing to AzuriteLang

Thank you for your interest in contributing!

## Getting Started

```bash
git clone https://github.com/AzuriteLang/AzuriteLang
cd AzuriteLang
cargo build --release --features llvm
cargo test
```

## Project Structure

```
azurite_lexer/      # Lexer (tokenizer)
azurite_parser/     # Parser + AST
azurite_checker/    # Type checker
azurite_codegen/    # LLVM codegen
azurite_errors/     # Error messages
azurite_resolver/   # Package manager
azurite_cli/        # CLI
azurite_test/       # Integration tests
```

## How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Code Style

- Rust edition 2021
- No `unwrap()` — use `?`, `.map_err()`, or `expect()` with a message
- All tests must pass: `cargo test --features llvm`
- No warnings: `cargo build --features llvm`

## Testing

```bash
cargo test --features llvm
```

## License

By contributing, you agree that your contributions will be licensed under the GPL v3.
