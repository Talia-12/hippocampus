use axum::{
	Json,
	extract::{Path, State},
};
use axum_extra::extract::Query;
use std::sync::Arc;
use tracing::{debug, info, instrument};

use crate::errors::ApiError;
use crate::models::Card;
use crate::repo;
use crate::{db::DbPool, models::ItemId};
use crate::{
	dto::{CreateCardDto, GetQueryDto, SortPositionAction},
	models::CardId,
};

/// Handler for creating a new card for an item
///
/// This function handles POST requests to `/items/{item_id}/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to create a card for
/// * `payload` - The request payload containing the card creation data
///
/// ### Returns
///
/// The newly created card as JSON
#[instrument(skip(pool), fields(item_id = %item_id, card_index = %payload.card_index, priority = %payload.priority))]
pub async fn create_card_handler(
	// Extract the database pool from the application state
	State(pool): State<Arc<DbPool>>,
	// Extract the item ID from the URL path
	Path(item_id): Path<ItemId>,
	// Extract and deserialize the JSON request body
	Json(payload): Json<CreateCardDto>,
) -> Result<Json<Card>, ApiError> {
	info!("Creating new card for item");

	// First check if the item exists
	let item = repo::get_item(&pool, &item_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	// Call the repository function to create the card
	let card = repo::create_card(&pool, &item.get_id(), payload.card_index, payload.priority)
		.await
		.map_err(ApiError::Database)?;

	info!("Successfully created card with id: {}", card.get_id());

	// Return the created card as JSON
	Ok(Json(card))
}

/// Handler for retrieving a specific card
///
/// This function handles GET requests to `/cards/{id}`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the card to retrieve, extracted from the URL path
///
/// ### Returns
///
/// The requested card as JSON, or null if not found
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn get_card_handler(
	// Extract the database pool from the application state
	State(pool): State<Arc<DbPool>>,
	// Extract the card ID from the URL path
	Path(card_id): Path<CardId>,
	// Extract query parameters
	Query(query): Query<GetQueryDto>,
) -> Result<Json<serde_json::Value>, ApiError> {
	debug!("Getting card");

	// `repo::get_card` is the canonical cache-aware read path — it ensures
	// `card_data` is fresh via a single atomic SELECT + staleness filter and
	// returns the resulting card. `?` uses `From<CardFetchError> for ApiError`
	// to route event-chain errors to `CardEventChainFailed` (500 with a
	// distinct message) instead of collapsing them into `Database` (500
	// "Internal server error"), so operators can tell registry/data drift
	// from plain DB failures.
	let card = repo::get_card(&pool, &card_id).await?;

	match card {
		Some(card) => {
			debug!("Card found with id: {}", card.get_id());
			let json = if query.split_priority.unwrap_or(false) {
				serde_json::to_value(&card).expect("Card serialization should never fail")
			} else {
				card.to_json_hide_priority_offset()
			};
			Ok(Json(json))
		}
		None => {
			debug!("Card not found");
			Ok(Json(serde_json::Value::Null))
		}
	}
}

/// Handler for listing all cards with optional filtering
///
/// This function handles GET requests to `/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `query` - Query parameters for filtering the results
///
/// ### Returns
///
/// A list of cards matching the filter criteria as JSON
#[instrument(skip(pool, query))]
pub async fn list_cards_handler(
	// Extract the database connection pool from the application state
	State(pool): State<Arc<DbPool>>,
	// Extract and parse query parameters
	Query(query): Query<GetQueryDto>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
	debug!("Listing cards with filters: {:?}", query);

	// `repo::list_cards` is the cache-aware list: it scopes `ensure_list_cards_cache`
	// to the request's filter before returning. `?` uses the typed
	// `CardFetchError → ApiError` conversion (see card_handlers.rs `get_card_handler`
	// comment) to preserve event-chain error attribution at the HTTP boundary.
	let cards = repo::list_cards(&pool, &query).await?;

	info!("Retrieved {} cards", cards.len());

	let split = query.split_priority.unwrap_or(false);
	let json_cards: Vec<serde_json::Value> = cards
		.iter()
		.map(|card| {
			if split {
				serde_json::to_value(card).expect("Card serialization should never fail")
			} else {
				card.to_json_hide_priority_offset()
			}
		})
		.collect();

	// Return the list of cards as JSON
	Ok(Json(json_cards))
}

/// Handler for listing cards for a specific item
///
/// This function handles GET requests to `/items/{item_id}/cards`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `item_id` - The ID of the item to get cards for
///
/// ### Returns
///
/// A list of cards for the specified item as JSON
#[instrument(skip(pool), fields(item_id = %item_id))]
pub async fn list_cards_by_item_handler(
	// Extract the database pool from the application state
	State(pool): State<Arc<DbPool>>,
	// Extract the item ID from the URL path
	Path(item_id): Path<ItemId>,
	// Extract query parameters
	Query(query): Query<GetQueryDto>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
	debug!("Listing cards for item");

	// First check if the item exists
	let _item = repo::get_item(&pool, &item_id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	// Call the cache-aware repo wrapper, which ensures all stale `card_data`
	// caches are recomputed before returning. `?` uses the typed
	// `CardFetchError → ApiError` conversion (see `get_card_handler` comment).
	let cards = repo::list_cards_by_item(&pool, &item_id).await?;

	info!("Retrieved {} cards for item {}", cards.len(), item_id);

	let split = query.split_priority.unwrap_or(false);
	let json_cards: Vec<serde_json::Value> = cards
		.iter()
		.map(|card| {
			if split {
				serde_json::to_value(card).expect("Card serialization should never fail")
			} else {
				card.to_json_hide_priority_offset()
			}
		})
		.collect();

	// Return the list of cards as JSON
	Ok(Json(json_cards))
}

/// Handler for updating a card's suspension state
///
/// This function handles POST requests to `/cards/{id}/suspend`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the card to update
/// * `payload` - The request payload containing the new suspension state
///
/// ### Returns
///
/// The updated card as JSON
#[instrument(skip(pool), fields(card_id = %id))]
pub async fn suspend_card_handler(
	// Extract the database pool from the application state
	State(pool): State<Arc<DbPool>>,
	// Extract the card ID from the URL path
	Path(id): Path<CardId>,
	// Extract and deserialize the JSON request body
	Json(payload): Json<bool>,
) -> Result<(), ApiError> {
	if payload {
		debug!("Suspending card");
	} else {
		debug!("Resuming card");
	}

	repo::set_card_suspended(&pool, &id, payload)
		.await
		.map_err(ApiError::Database)?;

	if payload {
		info!("Successfully suspended card");
	} else {
		info!("Successfully resumed card");
	}

	Ok(())
}

/// Handler for updating a card's priority
///
/// This function handles PATCH requests to `/cards/{id}/priority`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `id` - The ID of the card to update
/// * `payload` - The request payload containing the new priority
///
/// ### Returns
///
/// The updated card as JSON
#[instrument(skip(pool), fields(card_id = %id, priority = %priority))]
pub async fn update_card_priority_handler(
	// Extract the database pool from the application state
	State(pool): State<Arc<DbPool>>,
	// Extract the card ID from the URL path
	Path(id): Path<CardId>,
	// Extract and deserialize the JSON request body
	Json(priority): Json<f32>,
) -> Result<Json<serde_json::Value>, ApiError> {
	info!("Updating card priority");

	// Check if the priority is valid
	if priority < 0.0 || priority > 1.0 {
		return Err(ApiError::InvalidPriority(format!(
			"Priority must be between 0 and 1, got {}",
			priority
		)));
	}

	// Existence pre-check is the only thing that lets us map missing cards
	// to 404 instead of the 500 that `update_card_priority`'s internal
	// "Card not found" anyhow would produce. Use the raw read (no daily
	// ensure, no cache pipeline) — priority writes don't need either, and
	// `update_card_priority` fires the offset ensure itself inside its
	// transaction.
	let _card = repo::get_card_raw(&pool, &id)
		.map_err(ApiError::Database)?
		.ok_or(ApiError::NotFound)?;

	// Call the repository function to update the card's priority (also resets priority_offset to 0)
	let card = repo::update_card_priority(&pool, &id, priority)
		.await
		.map_err(ApiError::Database)?;

	info!("Successfully updated card priority to {}", priority);

	// Return the updated card as JSON with hidden priority offset
	Ok(Json(card.to_json_hide_priority_offset()))
}

/// Handler for setting a card's sort position
///
/// This function handles PATCH requests to `/cards/{card_id}/sort_position`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `card_id` - The ID of the card to reposition
/// * `payload` - The sort position action (top, before, after)
///
/// ### Returns
///
/// The updated card as JSON
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn set_sort_position_handler(
	State(pool): State<Arc<DbPool>>,
	Path(card_id): Path<CardId>,
	Json(payload): Json<SortPositionAction>,
) -> Result<Json<serde_json::Value>, ApiError> {
	info!("Setting card sort position");

	let card = match payload {
		SortPositionAction::Top => repo::move_card_to_top(&pool, &card_id)
			.await
			.map_err(ApiError::Database)?,
		SortPositionAction::Bottom => repo::move_card_to_bottom(&pool, &card_id)
			.await
			.map_err(ApiError::Database)?,
		SortPositionAction::Before { card_id: target_id } => {
			repo::move_card_relative(&pool, &card_id, &target_id, true)
				.await
				.map_err(ApiError::Database)?
		}
		SortPositionAction::After { card_id: target_id } => {
			repo::move_card_relative(&pool, &card_id, &target_id, false)
				.await
				.map_err(ApiError::Database)?
		}
	};

	info!("Successfully set sort position for card {}", card_id);
	Ok(Json(card.to_json_hide_priority_offset()))
}

/// Handler for clearing a single card's sort position
///
/// This function handles DELETE requests to `/cards/{card_id}/sort_position`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `card_id` - The ID of the card to clear
///
/// ### Returns
///
/// An empty successful response
#[instrument(skip(pool), fields(card_id = %card_id))]
pub async fn clear_card_sort_position_handler(
	State(pool): State<Arc<DbPool>>,
	Path(card_id): Path<CardId>,
) -> Result<(), ApiError> {
	info!("Clearing card sort position");

	repo::clear_card_sort_position(&pool, &card_id)
		.await
		.map_err(ApiError::Database)?;

	info!("Successfully cleared sort position for card {}", card_id);
	Ok(())
}

/// Handler for clearing all sort positions
///
/// This function handles DELETE requests to `/cards/sort_positions`.
///
/// ### Arguments
///
/// * `pool` - The database connection pool
/// * `query` - Optional query filters to limit which cards have their sort positions cleared
///
/// ### Returns
///
/// An empty successful response
#[instrument(skip(pool))]
pub async fn clear_sort_positions_handler(
	State(pool): State<Arc<DbPool>>,
	Query(query): Query<GetQueryDto>,
) -> Result<(), ApiError> {
	info!("Clearing sort positions with filters: {:?}", query);

	repo::clear_sort_positions(&pool, &query)
		.await
		.map_err(ApiError::Database)?;

	info!("Successfully cleared sort positions");
	Ok(())
}

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;
