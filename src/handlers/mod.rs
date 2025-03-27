/// Web API Handlers
///
/// This module contains the handlers for the RESTful API endpoints.
/// Each handler is responsible for processing a specific type of HTTP request,
/// extracting the necessary data, calling the appropriate repository functions,
/// and returning a properly formatted response.

mod item_handlers;
mod item_type_handlers;
mod card_handlers;
mod tag_handlers;
mod review_handlers;

// Re-export all handlers
pub use item_handlers::*;
pub use item_type_handlers::*;
pub use card_handlers::*;
pub use tag_handlers::*;
pub use review_handlers::*; 