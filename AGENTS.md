# Agent Instructions

## Quick Start

```bash
# Run the app (requires docker-compose for observability)
docker compose up -d
cargo run

# Access GraphQL Playground at http://localhost:8000
```

## Database & SQLx

- **Database**: SQLite at `sqlite.db` (auto-created on startup)
- **Migrations**: Located in `migrations/`
- **SQLx CLI required**: Install with:
  ```bash
  cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,sqlite
  ```
- **Before running tests/linting**:
  ```bash
  ./scripts/init_db.sh  # Creates DB and runs migrations
  cargo sqlx prepare --check -- --bin axum-graphql  # Verify .sqlx cache
  ```
- **After changing queries**: Update `.sqlx/` cache with `cargo sqlx prepare`
- **When adding or modifying migrations**: Run `cargo sqlx prepare` to refresh
  the query cache after migrations are applied.

## Testing

- All tests use in-memory SQLite (`sqlite://:memory:`)
- Tests are integration tests in `tests/api/`
- Run tests: `cargo test`
- **Running a single test**: `cargo test --test <test_name>` (
  `cargo test --test api::users_get`, for example)
- Snapshot review: `cargo insta review`

## Linting & Formatting

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

Pre-commit hooks run: fmt, cargo-check, clippy, cargo-deny, typos,
gitleaks, yamlfmt

## Toolchain

- **Rust**: 1.96.1 (toolchain file), MSRV: 1.94.0
- **Components**: clippy, llvm-tools-preview, rustfmt

## Endpoints

| Service            | URL                           |
| ------------------ | ----------------------------- |
| GraphQL Playground | http://localhost:8000         |
| Metrics            | http://localhost:8889/metrics |
| Jaeger Query       | http://localhost:16686/search |
| Grafana            | http://localhost:3000         |

## Architecture

- **Entry point**: `src/main.rs` → `Application::build()` in `src/startup.rs`
- **GraphQL routes**: `src/router.rs`
- **Database**: `src/database.rs` with migrations auto-run on startup
- **Observability**: OpenTelemetry → OTLP Collector → Prometheus/Jaeger/Loki

## Just Commands

```bash
just              # List all commands
just coverage     # Generate coverage report with grcov
just comments     # Find TODOs/comments in Rust source
just expects      # Find .expect() and .unwrap() calls
```

Key shortcuts:

- `just` – lists all available shortcut commands (`coverage`, `comments`,
  `expects`, etc.).
