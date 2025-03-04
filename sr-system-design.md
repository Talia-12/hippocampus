# Spaced Repetition Server Design Specification

## 1. System Overview

The Spaced Repetition Server (SRS) is a Rust-based background service that provides centralized management of spaced repetition items across multiple applications and platforms. It serves as the backbone of a "Spaced Repetition OS" concept, allowing various applications to register, retrieve, and update spaced repetition items through a unified API.

### 1.1 Core Principles

- **Universal Item Storage**: Centralized storage for all spaced repetition items regardless of source
- **Extensible Item Types**: Support for diverse item types with custom fields and review methods
- **Flexible Scheduling**: Multiple scheduling algorithms appropriate for different item types
- **Multi-client Support**: API designed for integration with various applications
- **Synchronization**: Support for syncing between multiple devices
- **Performance**: Ability to scale to hundreds of thousands of items

### 1.2 Key Components

1. **Core Server**: Tokio-based async server handling requests and managing the database
2. **API Layer**: Hybrid GraphQL and minimal REST API
3. **Database**: SQLite with Diesel ORM for data persistence
4. **Scheduler**: FSRS algorithm and alternatives for different item types
5. **Type Registry**: System for registering and managing custom item types
6. **Sync Service**: Mechanism for synchronizing data between devices
7. **Media Storage**: System for managing media files associated with items

## 2. System Architecture

### 2.1 High-Level Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     Client Applications                      │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  │
│  │  Browser  │  │   Todo    │  │ Obsidian  │  │ Flashcard │  │
│  │ Extension │  │    App    │  │ Extension │  │    App    │  │
│  └───────────┘  └───────────┘  └───────────┘  └───────────┘  │
└──────────────────────────────────────────────────────────────┘
                          │
                          │ GraphQL / REST API
                          ▼
┌──────────────────────────────────────────────────────────────┐
│                 Spaced Repetition Server                     │
│                                                              │
│  ┌────────────┐   ┌────────────┐   ┌────────────────────┐    │
│  │API Service │───│ Core Logic │───│ Type Registry      │    │
│  │            │   │            │   │                    │    │
│  │ - GraphQL  │   │ - Item     │   │ - Custom Fields    │    │
│  │ - REST     │   │   Manager  │   │ - Review Interfaces│    │
│  └────────────┘   │ - Scheduler│   │ - Algorithms       │    │
│                   └────────────┘   └────────────────────┘    │
│                         │                                    │
│                         ▼                                    │
│  ┌──────────────────────────────────────┐  ┌──────────────┐  │
│  │           Database                   │  │Media Storage │  │
│  │                                      │  │              │  │
│  │ - Items                              │  │ - Images     │  │
│  │ - Relationships                      │  │ - Audio      │  │
│  │ - Tags                               │  │ - Video      │  │
│  │ - Review History                     │  │              │  │
│  └──────────────────────────────────────┘  └──────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### 2.2 Component Interactions

- Client applications communicate with the server through GraphQL and REST API endpoints
- The API service validates requests and forwards them to the Core Logic
- The Core Logic handles business rules and interacts with the database
- The Type Registry manages custom item types and their associated behaviors
- Media Storage handles storage and retrieval of media files

## 3. Data Model

### 3.1 Core Database Schema

The database schema should be implemented using the following tables:

#### 3.1.1 Items Table
- Primary table storing all spaced repetition items
- Contains:
  - UUID primary key
  - Title
  - Type ID (references the item type registry)
  - JSON content field (stores type-specific data)
  - Created/updated timestamps
  - Next review and last review timestamps
  - Priority field (integer, used for sorting)
  - Data version (integer, for conflict resolution)

#### 3.1.2 Relationships Table
- Stores parent-child relationships and arbitrary links between items
- Contains:
  - UUID primary key
  - From item ID (references items table)
  - To item ID (references items table)
  - Relationship type ("parent-child" or "link")
  - Created timestamp
  - Unique constraint on (from_item_id, to_item_id, relationship_type)

#### 3.1.3 Tags Table
- Stores tags that can be applied to items
- Contains:
  - UUID primary key
  - Name (unique)

#### 3.1.4 Item Tags Junction Table
- Many-to-many relationship between items and tags
- Contains:
  - Item ID (references items table)
  - Tag ID (references tags table)
  - Primary key on (item_id, tag_id)

#### 3.1.5 Review History Table
- Stores review history for each item
- Contains:
  - UUID primary key
  - Item ID (references items table)
  - Review timestamp
  - Rating given during review (integer)
  - Review duration in milliseconds
  - Scheduler-specific data (JSON)
  - Device ID (for tracking which device performed the review)

#### 3.1.6 Item Types Registry Table
- Stores registered item types and their properties
- Contains:
  - ID (string primary key)
  - Name
  - Description (optional)
  - Schema (JSON Schema defining the structure of the content field)
  - Scheduler type
  - Review command (optional shell command for external review)
  - Created/updated timestamps

#### 3.1.7 Media Table
- Stores metadata about media files
- Contains:
  - UUID primary key
  - Filename
  - Content type
  - Size in bytes
  - Hash (for deduplication)
  - Created timestamp

#### 3.1.8 Item Media Junction Table
- Many-to-many relationship between items and media
- Contains:
  - Item ID (references items table)
  - Media ID (references media table)
  - Primary key on (item_id, media_id)

#### 3.1.9 Sync State Table
- Stores synchronization state for each device
- Contains:
  - Device ID (string primary key)
  - Last sync timestamp
  - Device name

### 3.2 Item Type Definition Schema

Item types should be defined using a JSON configuration format. The schema must include:

1. **ID**: Unique identifier for the item type
2. **Name**: Human-readable name
3. **Description**: Optional description of the item type
4. **Schema**: JSON Schema defining the structure of the item content
5. **Scheduler Type**: Which scheduling algorithm to use
6. **Review Interface**: How to review the item (default or custom)
7. **Review Command**: Optional shell command for external review (only one of this and Review Interface should be present)

### 3.3 Example Item Types

#### 3.3.1 Basic Flashcard
```json
{
  "id": "flashcard",
  "name": "Basic Flashcard",
  "description": "A basic front-back flashcard",
  "schema": {
    "type": "object",
    "required": ["front", "back"],
    "properties": {
      "front": { "type": "string" },
      "back": { "type": "string" }
    }
  },
  "scheduler_type": "fsrs",
  "review_interface": "default_flashcard"
}
```

#### 3.3.2 Cloze Deletion
```json
{
  "id": "cloze",
  "name": "Cloze Deletion",
  "description": "A text with hidden parts to recall",
  "schema": {
    "type": "object",
    "required": ["text", "clozes"],
    "properties": {
      "text": { "type": "string" },
      "clozes": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "start": { "type": "integer" },
            "end": { "type": "integer" },
            "hint": { "type": "string" }
          }
        }
      }
    }
  },
  "scheduler_type": "fsrs",
  "review_interface": "cloze_view"
}
```

#### 3.3.3 Todo Item
```json
{
  "id": "todo",
  "name": "Todo Item",
  "description": "A task to be completed",
  "schema": {
    "type": "object",
    "required": ["description", "status"],
    "properties": {
      "description": { "type": "string" },
      "status": { 
        "type": "string",
        "enum": ["pending", "completed", "deferred"] 
      },
      "due_date": { "type": "string", "format": "date-time" },
      "priority": { 
        "type": "integer",
        "minimum": 1,
        "maximum": 5 
      },
      "recurrence": {
        "type": "object",
        "properties": {
          "pattern": { 
            "type": "string",
            "enum": ["daily", "weekly", "monthly", "yearly", "custom"] 
          },
          "interval": { "type": "integer" },
          "custom_rule": { "type": "string" }
        }
      }
    }
  },
  "scheduler_type": "simple_defer",
  "review_interface": "todo_view"
}
```

#### 3.3.4 Web Page Extract
```json
{
  "id": "web_extract",
  "name": "Web Page Extract",
  "description": "An extract from a web page for incremental reading",
  "schema": {
    "type": "object",
    "required": ["url", "title", "selection"],
    "properties": {
      "url": { "type": "string", "format": "uri" },
      "title": { "type": "string" },
      "selection": {
        "type": "object",
        "properties": {
          "text": { "type": "string" },
          "xpath": { "type": "string" },
          "css_path": { "type": "string" },
          "offset": { "type": "integer" }
        }
      },
      "notes": { "type": "string" }
    }
  },
  "scheduler_type": "fsrs",
  "review_command": "open-browser \"{{url}}\" --highlight \"{{selection.xpath}}\""
}
```

## 4. API Design

### 4.1 GraphQL API

The GraphQL API should serve as the primary interface for client applications. It should include the following operations:

#### 4.1.1 Queries
- **Item Queries**:
  - Get a single item by ID
  - Query items with filtering, sorting, and pagination
  - Get items due for review with filters for types, tags, and priority
- **Tag Queries**:
  - Get all tags
- **Type Registry Queries**:
  - List all registered item types
  - Get a specific item type by ID
- **Review History Queries**:
  - Get review history for an item or time period
- **Statistics Queries**:
  - Get usage statistics and analytics

#### 4.1.2 Mutations
- **Item Mutations**:
  - Create a new item
  - Update an existing item
  - Delete an item
- **Relationship Mutations**:
  - Create a relationship between items
  - Delete a relationship
- **Tag Mutations**:
  - Create a new tag
  - Delete a tag
  - Add/remove tags from items
- **Review Mutations**:
  - Record a review for an item
- **Type Registry Mutations**:
  - Register a new item type
  - Update an existing item type
- **Sync Mutations**:
  - Request synchronization between devices

#### 4.1.3 Subscriptions
- **Item Changed**: Notify when items are created, updated, or deleted (there should be some way to do this for a filtered view of the items)
- **Review Recorded**: Notify when a review is recorded (again, this should allow for filtering)
- **Sync Completed**: Notify when synchronization is completed

#### 4.1.4 Type Definitions
The GraphQL schema should define types that match the database schema, including:
- Item
- ItemType
- Tag
- Relationship
- ReviewRecord
- Statistics
- Various input types for mutations
- Pagination types (connections, edges, etc.)
- Event types for subscriptions

### 4.2 REST API

The REST API should be minimal, primarily focused on allowing simple integrations for adding new items:

- **POST /api/items**: Create a new item
- **POST /api/reviews**: Record a review
- **POST /api/sync**: Request synchronization

### 4.3 API Authentication

As this is primarily a personal system, authentication should be simple:
- API token-based authentication for external clients
- Local IPC connections should be trusted by default
- Configuration option to disable authentication for trusted networks

## 5. Scheduling System

### 5.1 Scheduler Interface

The scheduling system should be modular, with a common interface that allows different algorithms to be used for different item types:

```rust
// Example interface - implementation can vary
pub trait Scheduler {
    fn schedule_review(
        &self, 
        item_id: Uuid,
        review_history: &[ReviewRecord],
        rating: i32,
        review_time: DateTime<Utc>,
    ) -> SchedulingResult;
}

pub struct SchedulingResult {
    pub next_review: DateTime<Utc>,
    pub scheduler_data: serde_json::Value,
    pub retention: Option<f64>,  // Estimated retention at next review
}
```

### 5.2 Scheduler Implementations

#### 5.2.1 FSRS Scheduler
- Implement the Free Spaced Repetition Scheduler (FSRS) algorithm
- Use the `fsrs-rs` crate for core algorithm logic
- Store algorithm parameters and state in the scheduler_data field
- Calculate retention probability for optimal scheduling

#### 5.2.2 Simple Defer Scheduler
- Implement a simple scheduler for non-learning items like todos
- Use rating to determine defer period:
  - Rating 1: Defer 1 day
  - Rating 2: Defer 3 days
  - Rating 3: Defer 7 days
  - Rating 4: Defer 1.3 times as long as previous defer
  - Rating 5: Defer 1.7 times as long as previous defer
- No retention calculation needed

### 5.3 Scheduler Registration

The system should include a registry for schedulers:
- Register default schedulers at startup
- Allow new schedulers to be registered at runtime
- Map scheduler types to their implementations
- Retrieve the appropriate scheduler for each item type

## 6. Synchronization System

### 6.1 Sync Protocol

The sync protocol should be based on a central server model:

1. Client sends last sync timestamp to server
2. Server returns all items changed since that timestamp
3. Client sends its changes to server
4. Server applies changes, resolving conflicts

Each sync request should include:
- Device ID
- Last sync timestamp
- List of changes (creates, updates, deletes)

### 6.2 Conflict Resolution

For conflict resolution, use a data versioning approach:

1. Each item has a `data_version` counter
2. Every modification increments this counter
3. When syncing, the higher version wins
4. If versions are equal but content differs, most recent `updated_at` wins
5. For non-conflicting fields (different fields modified), merge the changes

### 6.3 Sync Process

The sync process should follow these steps:

1. Client initiates sync with server
2. Server checks for changes since last sync
3. Server sends changes to client
4. Client applies server changes locally
5. Client sends its changes to server
6. Server applies client changes
7. Both update last sync timestamp

## 7. Media Storage System

### 7.1 Structure

The media storage system should manage files associated with items:

1. Files should be stored in a structured directory
2. Filenames should be based on hashes to avoid duplicates
3. A database table should track metadata and relationships to items

Directory structure:
```
media/
├── images/
│   ├── ab/
│   │   └── ab123456789abcdef.jpg
│   └── cd/
│       └── cd987654321fedcba.png
├── audio/
│   └── ...
└── other/
    └── ...
```

### 7.2 Media Operations

The media service should provide operations for:

- Storing a new file
  - Calculate hash to check for duplicates
  - Determine appropriate category based on content type
  - Store file in correct location
  - Record metadata in database
- Linking media to items
  - Create association in the item_media table
- Retrieving a file
  - Look up metadata in database
  - Return file data, filename, and content type
- Managing orphaned files
  - Periodically check for files not linked to any items
  - Option to remove orphaned files or move to archive

## 8. Item Management

### 8.1 Core Operations

The item service should provide these core operations:

#### 8.1.1 Create Item
- Generate a new UUID
- Validate content against type schema
- Set created/updated timestamps
- Set initial data version to 1
- No next_review or last_review initially
- Add to database
- Establish relationships if specified
- Apply tags if specified

#### 8.1.2 Update Item
- Retrieve existing item
- Verify data version matches to prevent conflicts
- Update specified fields only
- Validate content against type schema if changed
- Update timestamp
- Increment data version
- Save to database

#### 8.1.3 Delete Item
- Remove item from database
- Cascade delete relationships and tag associations

#### 8.1.4 Create Relationship
- Establish parent-child or link relationship between items
- Enforce relationship type constraints

#### 8.1.5 Get Due Items
- Find items due for review (next_review ≤ current time)
- Filter by types, tags, priority
- Order by priority, then next_review
- Apply pagination/limits

### 8.2 Required Filters and Sorts

The item query system should support:

- Filtering by:
  - Item type
  - Tags
  - Parent item
  - Priority range
  - Due date range
  - Text search
- Sorting by:
  - Title
  - Created date
  - Updated date
  - Next review date
  - Priority
- Pagination using cursor-based approach

## 9. Type Registry

### 9.1 Type Registration

The type registry should manage custom item types:

#### 9.1.1 Register Type
- Validate type definition
  - Ensure schema is valid JSON Schema
  - Ensure scheduler type exists
- Add to type registry database

#### 9.1.2 Get Type
- Retrieve type definition by ID
- Return all type properties

#### 9.1.3 Validate Item Content
- Load type schema
- Validate content against schema using JSON Schema validation
- Return validation errors if any

#### 9.1.4 Get Scheduler
- Return the appropriate scheduler for a given scheduler type

### 9.2 Default Types

The system should register these default types at startup:
- Basic Flashcard
- Cloze Deletion
- Todo Item
- Web Page Extract

## 10. Review System

### 10.1 Recording Reviews

The review service should handle the process of recording reviews:

1. Get the item being reviewed
2. Get the item type
3. Get the appropriate scheduler
4. Get review history for the item
5. Apply the scheduling algorithm to determine next review time
6. Create a review record with:
   - Review time
   - Rating
   - Duration (if provided)
   - Scheduler-specific data
   - Device ID
7. Update the item's next_review and last_review timestamps
8. Increment the item's data_version

### 10.2 Review History

The system should provide access to review history:
- Filter by item ID
- Filter by date range
- Return ratings, times, and scheduler data
- Calculate statistics based on history

### 10.3 Statistics Collection

The statistics service should collect and provide:
- Total number of items
- Items by type
- Reviews per day/week/month
- Average rating
- Estimated retention rate
- Number of items due today/this week
- Review time distribution

## 11. Server Implementation

### 11.1 Core Server

The server should be implemented using Tokio for asynchronous processing:

#### 11.1.1 Server Initialization
1. Set up database connection pool
2. Run database migrations
3. Initialize services:
   - Type Registry
   - Item Service
   - Review Service
   - Media Service
   - Statistics Service
4. Register default item types
5. Start API servers
6. Start background tasks

#### 11.1.2 API Servers
1. GraphQL server using async-graphql and warp
   - Create schema with resolvers for all operations
   - Set up endpoint for GraphQL requests
   - Configure WebSocket for subscriptions
2. REST server using warp
   - Set up routes for minimal REST API
   - Parse and validate requests
   - Forward to core services

#### 11.1.3 Background Tasks
1. Periodic housekeeping:
   - Clean up orphaned media files
   - Update statistics
   - Run database optimizations
2. Optional notification system for due items

### 11.2 Configuration

The server should be configurable via:
1. Configuration file (TOML or YAML)
2. Environment variables
3. Command-line arguments

Configuration options should include:
- Database path
- Media storage path
- Host/ports for API servers
- Authentication settings
- Logging level
- Scheduler parameters

## 12. Client Integration Examples

### 12.1 Browser Extension Integration

A browser extension might integrate with the SR Server by:
1. Capturing selected text on a webpage
2. Determining XPath or CSS selector for the selection
3. Creating a web_extract item via GraphQL API
4. Showing notification of successful creation

### 12.2 Todo App Integration

A todo app might integrate by:
1. Creating todo items with description, due date, priority, and recurrence
2. Fetching due todos for today
3. Marking todos as completed or deferring them
4. Using the simple_defer scheduler to handle deferred todos

### 12.3 Obsidian Extension Integration

An Obsidian extension might integrate by:
1. Adding commands to mark notes or sections for review
2. Creating flashcards or cloze deletions from note content
3. Syncing with the SR Server
4. Adding review status indicators to notes

### 12.4 Flashcard Interface

The main flashcard review interface should:
1. Fetch due items from the server
2. Present items for review based on type
3. Record ratings and review time
4. Support different item types with appropriate review interfaces
5. Show statistics and progress

## 13. Implementation Plan

### 13.1 Phase 1: Core Infrastructure
1. Set up project structure and dependencies
2. Implement database schema and migrations
3. Create basic API framework
4. Implement core item management

### 13.2 Phase 2: Scheduling and Review
1. Implement FSRS scheduler
2. Implement simple defer scheduler
3. Create review recording system
4. Build flashcard review interface

### 13.3 Phase 3: Type System and Extensions
1. Implement type registry
2. Add default item types
3. Create media storage system
4. Implement custom review interfaces

### 13.4 Phase 4: Sync and Client Integration
1. Implement sync protocol
2. Create client libraries
3. Create Android Wear -> database system

### 13.5 Phase 5: Extra Features
1. Implement statistics collection
2. Develop browser extension integration

## 14. Performance Considerations

### 14.1 Database Optimization
1. Use appropriate indexes:
   - Items table: next_review, type_id, priority
   - Relationships table: from_item_id, to_item_id
   - Review history: item_id, review_time
2. Consider using WAL mode for SQLite
3. Implement batch operations for bulk updates

### 14.2 Scaling Strategy
1. Initial implementation with SQLite for simplicity
2. Design to allow migration to PostgreSQL if needed
3. Use connection pooling to handle concurrent requests
4. Implement pagination for large result sets
5. Use efficient queries for due item calculation

### 14.3 Memory Management
1. Avoid loading entire dataset into memory
2. Stream large results
3. Use appropriate buffer sizes for file operations
4. Implement cleanup routines for temporary files

## 15. Testing Strategy

### 15.1 Unit Tests
1. Test each service component in isolation
2. Mock dependencies to ensure focused testing
3. Ensure high coverage of core functionality

### 15.2 Integration Tests
1. Test API endpoints with real database
2. Verify correct interaction between components
3. Test synchronization between multiple clients

### 15.3 Performance Tests
1. Test with large datasets (100,000+ items)
2. Measure response times for common operations
3. Identify bottlenecks and optimize

### 15.4 Security Tests
1. Validate input sanitization
2. Test authentication mechanisms
3. Verify proper handling of file uploads

## 16. Security Considerations

### 16.1 Data Security
1. Use parameterized queries to prevent SQL injection
2. Validate all input, especially file uploads
3. Implement proper error handling to avoid information leakage

### 16.2 API Security
1. Use HTTPS for all external connections
2. Implement API token authentication
3. Verify origin for cross-origin requests

### 16.3 File Security
1. Validate uploaded files
2. Use secure random filenames
3. Implement proper permissions on storage directories

## 17. Deployment Considerations

### 17.1 Installation
1. Provide simple installation script
2. Document dependencies
3. Include database initialization

### 17.2 Upgrades
1. Include database migration system
2. Ensure backward compatibility
3. Document upgrade procedures

### 17.3 Backup
1. Implement backup command
2. Document restore procedures
3. Ensure media files are included in backups
