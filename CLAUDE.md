# Hippocampus Project Guide

## Build/Test Commands
```bash
# Build the project
cargo build

# Run the project
cargo run

# Run all tests
cargo test

# Run a single test (by name)
cargo test test_name

# Run all integration tests
cargo test --test integration

# Run a specific integration test
cargo test --test integration test_create_item

# Run tests with test feature enabled
cargo test --features test

# Run database migrations (must specify --database-url for each database)
diesel migration run --database-url srs_server.db
diesel migration run --database-url test_database.db
```

## Code Style Guidelines
- **Naming**: snake_case for variables/functions, CamelCase for types/structs
- **Error Handling**: Return `Result<T, ApiError>`, use `map_err` for conversions
- **Documentation**: Use `///` doc comments for all public items
- **Testing**: Unit tests in same file as code, integration tests in `tests/`
- **Organization**: Group imports by category (std lib, external, internal)
- **Types**: Strong typing with getters/setters for all struct fields
- **Documentation**: All functions have doc comments with purpose, args, and returns
- **Database**: Use Diesel ORM with properly separated models and repository functions

## Testing Philosophy
- **Build new features with red/green TDD**: Write a failing test that specifies the new behavior (red), then implement the minimum code to make it pass (green), then refactor.
- **Prefer proptests over unit tests**: Reach for property-based tests first; fall back to unit tests only when a property is hard to express or when locking in a specific concrete case.
- **Tests should fully specify the program**: The goal is a test set so complete that if all non-test code were deleted, the program could be reconstructed from scratch by implementing against the tests. When adding or changing behavior, ask whether the tests alone would force that behavior to exist.
- **Proptest regressions become unit tests**: Whenever a proptest fails and proptest writes a regression seed (e.g. into `proptest-regressions/`), also add a dedicated unit test that exercises that specific failing input. The regression file guards the seed; the unit test makes the failure mode explicit, named, and immediately visible in the test output if it ever recurs.
