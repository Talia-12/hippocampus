# Hippocampus: A Spaced Repetition System

Hippocampus is a Rust-based spaced repetition server designed to help users memorize information more effectively by scheduling reviews at optimal intervals. The name "Hippocampus" refers to the part of the brain involved in memory formation, which is fitting for a system designed to enhance learning and retention.

## Features

- **Core Spaced Repetition Functionality**: Schedule items for review using scientifically-backed spaced repetition algorithms
- **Multiple Item Types**: Support for different kinds of learning material (flashcards, etc.)
- **Tagging System**: Organize items with tags, including hidden system tags
- **RESTful API**: Complete API for item management, reviews, and scheduling
- **SQLite Database**: Persistent storage with Diesel ORM

## Project Structure

- `src/`: Source code
  - `main.rs`: Application entry point
  - `lib.rs`: Core library functionality
  - `db.rs`: Database connection management
  - `models.rs`: Data structures representing items and reviews
  - `repo.rs`: Repository layer for database operations
  - `schema.rs`: Diesel-generated database schema
- `migrations/`: Database migration files
- `tests/`: Integration tests

## API Endpoints

The application exposes a RESTful API with the following endpoints:

- **Item Types**
  - `GET /item_types`: List all item types
  - `GET /item_types/{id}`: Get a specific item type
  - `POST /item_types`: Create a new item type
  - `GET /item_types/{id}/items`: List items of a specific type

- **Items**
  - `GET /items`: List all items
  - `GET /items/{id}`: Get a specific item
  - `POST /items`: Create a new item
  - `GET /items/{id}/cards`: List cards for a specific item

- **Cards**
  - `GET /cards`: List all cards (with optional filtering)
  - `GET /cards/{id}`: Get a specific card

- **Reviews**
  - `POST /reviews`: Record a review for a card

- **Tags**
  - `GET /tags`: List all tags
  - `POST /tags`: Create a new tag
  - `POST /items/{item_id}/tags/{tag_id}`: Add a tag to an item
  - `DELETE /items/{item_id}/tags/{tag_id}`: Remove a tag from an item

## Data Model

- **Item Types**: Define different types of items (e.g., flashcards, cloze deletions)
- **Items**: The basic unit of information to be remembered
- **Cards**: Individual review units derived from items
- **Reviews**: Records of review sessions with ratings
- **Tags**: Labels for organizing and filtering items

## Getting Started

### Prerequisites

- Rust (2024 edition)
- SQLite

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/hippocampus.git
   cd hippocampus
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run the database migrations:
   ```bash
   diesel migration run
   ```

4. Run the server:
   ```bash
   cargo run
   ```

The server will start on `localhost:3000`.

## Development

### Building and Testing

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

### Code Style Guidelines

- **Naming**: snake_case for variables/functions, CamelCase for types/structs
- **Error Handling**: Return `Result<T, ApiError>`, use `map_err` for conversions
- **Documentation**: Use `///` doc comments for all public items
- **Testing**: Unit tests in same file as code, integration tests in `tests/`
- **Organization**: Group imports by category (std lib, external, internal)
- **Types**: Strong typing with getters/setters for all struct fields
- **Documentation**: All functions have doc comments with purpose, args, and returns
- **Database**: Use Diesel ORM with properly separated models and repository functions

## Future Development

Based on the system design documents, future development may include:

- FSRS (Free Spaced Repetition Scheduler) implementation
- Multiple scheduling algorithms for different item types
- Synchronization between devices
- Media storage for images and other attachments
- GraphQL API alongside RESTful endpoints
- Extended statistics and analytics

## License

[MIT License](LICENSE)