# Hippocampus: A Spaced Repetition System

Hippocampus is designed primarily as an implementation of the idea Andy Matuschak called an [OS-level spaced repetition system](https://notes.andymatuschak.org/zNLoqjEVe5dheMKmTTyB9E3). It functions as a server running in the background which any app can push items to study into, and a frontend that will request cards for review from (I am in the process of building this frontend). Use cases which I intend to support are study with spaced repetition, incremental reading, and attenuating todos.

## Project Structure

- `src/`: Source code
  - `main.rs`: Application entry point
  - `lib.rs`: Core library functionality
  - `db.rs`: Database connection management
  - `models/`: Data structures representing items, cards, reviews, and tags
    - `item.rs`: Item model and related functions
    - `item_type.rs`: Item type definitions
    - `card.rs`: Card model for review scheduling
    - `review.rs`: Review records and rating processing
    - `tag.rs`: Tags for item organization
    - `item_tag.rs`: Relationship between items and tags
    - `json_value.rs`: Support for storing JSON data
  - `repo/`: Repository layer for database operations
  - `handlers/`: API request handlers
    - `item_handlers.rs`: Endpoints for managing items
    - `item_type_handlers.rs`: Endpoints for managing item types
    - `card_handlers.rs`: Endpoints for managing review cards
    - `review_handlers.rs`: Endpoints for recording reviews
    - `tag_handlers.rs`: Endpoints for tags and item tagging
  - `errors.rs`: Error handling
  - `dto.rs`: Data transfer objects for API
  - `schema.rs`: Diesel-generated database schema
- `migrations/`: Database migration files
- `tests/`: Integration tests

## API Endpoints

The application exposes a RESTful API with the following endpoints:

### Item Types
- `GET /item_types`: List all item types
- `POST /item_types`: Create a new item type
- `GET /item_types/{id}`: Get a specific item type
- `GET /item_types/{id}/items`: List items of a specific type

### Items
- `GET /items`: List all items
- `POST /items`: Create a new item
- `GET /items/{id}`: Get a specific item
- `GET /items/{id}/cards`: List cards for a specific item
- `POST /items/{id}/cards`: Create a new card for an item
- `GET /items/{item_id}/tags`: List all tags for an item
- `PUT /items/{item_id}/tags/{tag_id}`: Add a tag to an item
- `DELETE /items/{item_id}/tags/{tag_id}`: Remove a tag from an item

### Cards
- `GET /cards`: List all cards (with optional filtering)
- `GET /cards/{id}`: Get a specific card
- `GET /cards/{card_id}/reviews`: List all reviews for a card
- `PUT /cards/{card_id}/priority`: Update the priority of a card
- `GET /cards/{card_id}/tags`: List all tags for a card

### Reviews
- `POST /reviews`: Record a review for a card

### Tags
- `GET /tags`: List all tags
- `POST /tags`: Create a new tag

## Data Model

- **Item Types**: Define different types of items (e.g., flashcards, cloze deletions)
- **Items**: The basic unit of information to be remembered
- **Cards**: Individual review units derived from items with scheduling information
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
- **Database**: Use Diesel ORM with properly separated models and repository functions

## Future Development

Planned enhancements include:

- FSRS (Free Spaced Repetition Scheduler) implementation
- Multiple scheduling algorithms for different item types
- Synchronization between devices
- Media storage for images and other attachments
- GraphQL API alongside RESTful endpoints
- Extended statistics and analytics

## License

[Apache 2.0 + Common Clause License](LICENSE.txt)
