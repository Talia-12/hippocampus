use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::{CardEventFnName, ItemTypeId, OrderIndex};

/// Represents a card fetched event in the pipeline for an item type
///
/// Card fetched events are pure functions that transform card data when cards
/// are fetched. They form an ordered pipeline per item type.
#[derive(
	Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize,
)]
#[diesel(table_name = crate::schema::card_fetched_events)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CardFetchedEvent {
	/// The ID of the item type this event belongs to
	item_type_id: ItemTypeId,

	/// The position of this event in the pipeline.
	order_index: OrderIndex,

	/// The name of the function to execute
	function_name: CardEventFnName,
}

impl CardFetchedEvent {
	/// Creates a new card fetched event
	///
	/// ### Arguments
	///
	/// * `item_type_id` - The ID of the item type this event belongs to
	/// * `order_index` - The position of this event in the pipeline
	/// * `function_name` - The name of the function to execute
	///
	/// ### Returns
	///
	/// A new `CardFetchedEvent` instance
	pub fn new(
		item_type_id: ItemTypeId,
		order_index: OrderIndex,
		function_name: CardEventFnName,
	) -> Self {
		Self {
			item_type_id,
			order_index,
			function_name,
		}
	}

	/// Gets the item type ID
	///
	/// ### Returns
	///
	/// The ID of the item type this event belongs to
	pub fn get_item_type_id(&self) -> ItemTypeId {
		self.item_type_id.clone()
	}

	/// Gets the order index
	///
	/// ### Returns
	///
	/// The position of this event in the pipeline
	pub fn get_order_index(&self) -> OrderIndex {
		self.order_index
	}

	/// Gets the function name
	///
	/// ### Returns
	///
	/// The name of the function to execute
	pub fn get_function_name(&self) -> CardEventFnName {
		self.function_name.clone()
	}
}
