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

# Run database migrations
diesel migration run
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