use super::*;
use proptest::prelude::*;

fn arb_item_type_id() -> impl Strategy<Value = ItemTypeId> {
	"[a-zA-Z0-9_-]{1,32}".prop_map(ItemTypeId)
}

fn arb_tag_id() -> impl Strategy<Value = TagId> {
	"[a-zA-Z0-9_-]{1,32}".prop_map(TagId)
}

proptest! {
	/// `all_query` returns every todo of the given type regardless of due
	/// date or completion: the chosen item_type_id and tag_ids pass through
	/// unchanged, `suspended_filter` is `Include`, and no date-bound
	/// filters are applied.
	#[test]
	fn all_query_includes_every_todo(
		item_type_id in arb_item_type_id(),
		tag_ids in prop::collection::vec(arb_tag_id(), 0..5),
	) {
		let query = all_query(item_type_id.clone(), tag_ids.clone());

		prop_assert_eq!(query.item_type_id.as_ref(), Some(&item_type_id));
		prop_assert_eq!(&query.tag_ids, &tag_ids);
		prop_assert_eq!(query.suspended_filter, SuspendedFilter::Include);
		prop_assert!(query.next_review_before.is_none());
		prop_assert!(query.last_review_after.is_none());
		prop_assert!(query.suspended_after.is_none());
		prop_assert!(query.suspended_before.is_none());
	}
}
