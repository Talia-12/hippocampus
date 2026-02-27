use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::ItemId;

/// Represents a parent-child relationship between two items
///
/// This supports SuperMemo-style incremental reading workflows where
/// articles can be extracted into sections, cloze deletions, etc.
/// The graph must remain acyclic (enforced server-side).
#[derive(
	Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize,
)]
#[diesel(table_name = crate::schema::item_relations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ItemRelation {
	/// The ID of the parent item
	parent_item_id: ItemId,

	/// The ID of the child item
	child_item_id: ItemId,

	/// The type of relationship (e.g. "extract", "cloze", "simplify")
	relation_type: String,

	/// When this relation was created
	created_at: NaiveDateTime,
}

impl ItemRelation {
	/// Creates a new item relation
	///
	/// ### Arguments
	///
	/// * `parent_item_id` - The ID of the parent item
	/// * `child_item_id` - The ID of the child item
	/// * `relation_type` - The type of relationship
	///
	/// ### Returns
	///
	/// A new `ItemRelation` instance
	pub fn new(parent_item_id: ItemId, child_item_id: ItemId, relation_type: String) -> Self {
		Self {
			parent_item_id,
			child_item_id,
			relation_type,
			created_at: Utc::now().naive_utc(),
		}
	}

	/// Gets the parent item ID
	///
	/// ### Returns
	///
	/// The ID of the parent item in this relation
	pub fn get_parent_item_id(&self) -> ItemId {
		self.parent_item_id.clone()
	}

	/// Gets the child item ID
	///
	/// ### Returns
	///
	/// The ID of the child item in this relation
	pub fn get_child_item_id(&self) -> ItemId {
		self.child_item_id.clone()
	}

	/// Gets the relation type
	///
	/// ### Returns
	///
	/// The type of this relationship
	pub fn get_relation_type(&self) -> String {
		self.relation_type.clone()
	}

	/// Gets the creation timestamp as a DateTime<Utc>
	///
	/// ### Returns
	///
	/// The timestamp when this relation was created
	pub fn get_created_at(&self) -> DateTime<Utc> {
		DateTime::from_naive_utc_and_offset(self.created_at, Utc)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_item_relation() {
		let relation = ItemRelation::new(
			ItemId("parent-id".to_string()),
			ItemId("child-id".to_string()),
			"extract".to_string(),
		);

		assert_eq!(relation.get_parent_item_id().0, "parent-id");
		assert_eq!(relation.get_child_item_id().0, "child-id");
		assert_eq!(relation.get_relation_type(), "extract");
	}

	#[test]
	fn test_getters() {
		let relation = ItemRelation::new(
			ItemId("parent-123".to_string()),
			ItemId("child-456".to_string()),
			"cloze".to_string(),
		);

		assert_eq!(relation.get_parent_item_id().0, "parent-123");
		assert_eq!(relation.get_child_item_id().0, "child-456");
		assert_eq!(relation.get_relation_type(), "cloze");
		// created_at should be close to now
		let now = Utc::now();
		let diff = now - relation.get_created_at();
		assert!(diff.num_seconds() < 2, "created_at should be close to now");
	}

	#[test]
	fn test_clone_and_eq() {
		let relation = ItemRelation::new(
			ItemId("parent-id".to_string()),
			ItemId("child-id".to_string()),
			"extract".to_string(),
		);
		let cloned = relation.clone();
		assert_eq!(relation, cloned);
	}
}
