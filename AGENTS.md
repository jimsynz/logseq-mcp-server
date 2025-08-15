# LogSeq MCP Server - Agent Guidelines

## Build Commands
```bash
cargo build                    # Build the project
cargo test --lib              # Run unit tests
cargo test integration_tests -- --ignored  # Run integration tests (requires LogSeq running)
cargo test test_name -- --ignored --nocapture  # Run single test with output
cargo fmt                     # Format code
cargo clippy --all-targets --all-features -- -D warnings  # Lint code
cargo check                   # Check for compilation errors
```

## Code Style
- **Imports**: Group by std, external crates, then internal modules; alphabetical within groups
- **Error Handling**: Use `anyhow::Result` for main functions, custom errors with `thiserror`
- **Async**: All API calls use `async/await` with `tokio` runtime
- **Naming**: snake_case for functions/variables, PascalCase for types, SCREAMING_SNAKE_CASE for constants
- **Testing**: Integration tests marked with `#[ignore]`, create test data with `test-{uuid}-{description}` format
- **MCP Tools**: Return structured data via `serde_json::Value`, format output in tool handlers
- **Client**: Share `LogSeqClient` via `Arc` for thread safety