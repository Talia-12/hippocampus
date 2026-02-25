use super::*;
use crate::repo::tests::setup_test_db;
use crate::repo::create_item_type;
use crate::test_utils::{arb_json, arb_messy_string, dedup_names, json_approx_eq};
use proptest::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

// ============================================================================
// IR1: CRUD Roundtrip Properties
// ============================================================================

proptest! {
    /// IR1.1: create→get preserves title for arbitrary strings
    #[test]
    fn prop_ir1_1_create_get_preserves_title(title in "\\PC+") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let data = serde_json::json!({"key": "value"});

            let created = create_item(&pool, &item_type.get_id(), title.clone(), data).await.unwrap();
            let retrieved = get_item(&pool, &created.get_id()).unwrap().unwrap();

            assert_eq!(retrieved.get_title(), title);
        });
    }

    /// IR1.2: create→get preserves item_type
    #[test]
    fn prop_ir1_2_create_get_preserves_item_type(name in "\\PC+") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, name).await.unwrap();
            let data = serde_json::json!({"key": "value"});

            let created = create_item(&pool, &item_type.get_id(), "Title".to_string(), data).await.unwrap();
            let retrieved = get_item(&pool, &created.get_id()).unwrap().unwrap();

            assert_eq!(retrieved.get_item_type(), item_type.get_id());
        });
    }

    /// IR1.3: create→get preserves data for arbitrary JSON
    #[test]
    fn prop_ir1_3_create_get_preserves_data(data in arb_json()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();

            let created = create_item(&pool, &item_type.get_id(), "Title".to_string(), data.clone()).await.unwrap();
            let retrieved = get_item(&pool, &created.get_id()).unwrap().unwrap();

            assert!(json_approx_eq(&retrieved.get_data().0, &data),
                "Data mismatch:\n  stored:    {:?}\n  retrieved: {:?}",
                data, retrieved.get_data().0);
        });
    }

    /// IR1.4: create produces valid UUID
    #[test]
    fn prop_ir1_4_create_produces_valid_uuid(title in "\\PC+") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let data = serde_json::json!({"key": "value"});

            let item = create_item(&pool, &item_type.get_id(), title, data).await.unwrap();
            assert!(Uuid::parse_str(&item.get_id()).is_ok(),
                "ID should be valid UUID, got: {}", item.get_id());
        });
    }

    /// IR1.5: get_item returns None for arbitrary nonexistent IDs
    #[test]
    fn prop_ir1_5_get_nonexistent_returns_none(id in arb_messy_string()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let result = get_item(&pool, &id).unwrap();
            assert!(result.is_none(),
                "get_item should return None for nonexistent id={:?}", id);
        });
    }

    /// IR1.6: bulk create with deduplicated titles produces unique, retrievable items
    #[test]
    fn prop_ir1_6_bulk_create_unique_ids(
        titles in prop::collection::vec("\\PC+", 0..=100)
            .prop_map(dedup_names)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let count = titles.len();

            let mut created = Vec::with_capacity(count);
            for title in &titles {
                let data = serde_json::json!({"title": title});
                created.push(
                    create_item(&pool, &item_type.get_id(), title.clone(), data).await.unwrap()
                );
            }

            // All IDs are unique
            let ids: HashSet<_> = created.iter().map(|it| it.get_id()).collect();
            assert_eq!(ids.len(), count,
                "Expected {} unique IDs, got {}", count, ids.len());

            // Each created item is retrievable
            for item in &created {
                let retrieved = get_item(&pool, &item.get_id())
                    .unwrap()
                    .expect("created item should be retrievable");
                assert_eq!(retrieved.get_id(), item.get_id());
                assert_eq!(retrieved.get_title(), item.get_title());
            }
        });
    }
}

// ============================================================================
// IR2: Update Properties
// ============================================================================

proptest! {
    /// IR2.1: update(title=Some(t), data=None) changes title, preserves data
    #[test]
    fn prop_ir2_1_update_title_preserves_data(
        orig_title in "\\PC+",
        new_title in "\\PC+",
        data in arb_json(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();

            let created = create_item(&pool, &item_type.get_id(), orig_title, data.clone()).await.unwrap();
            let updated = update_item(&pool, &created.get_id(), Some(new_title.clone()), None).await.unwrap();

            assert_eq!(updated.get_title(), new_title);
            assert!(json_approx_eq(&updated.get_data().0, &data),
                "Data should be preserved after title-only update");
        });
    }

    /// IR2.2: update(title=None, data=Some(d)) changes data, preserves title
    #[test]
    fn prop_ir2_2_update_data_preserves_title(
        title in "\\PC+",
        orig_data in arb_json(),
        new_data in arb_json(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();

            let created = create_item(&pool, &item_type.get_id(), title.clone(), orig_data).await.unwrap();
            let updated = update_item(&pool, &created.get_id(), None, Some(new_data.clone())).await.unwrap();

            assert_eq!(updated.get_title(), title);
            assert!(json_approx_eq(&updated.get_data().0, &new_data),
                "Data should match after data-only update");
        });
    }

    /// IR2.3: update(None, None) is content-identity: title and data unchanged
    #[test]
    fn prop_ir2_3_update_none_none_identity(title in "\\PC+", data in arb_json()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();

            let created = create_item(&pool, &item_type.get_id(), title.clone(), data.clone()).await.unwrap();
            let updated = update_item(&pool, &created.get_id(), None, None).await.unwrap();

            assert_eq!(updated.get_title(), title);
            assert!(json_approx_eq(&updated.get_data().0, &data),
                "Data should be unchanged after no-op update");
        });
    }

    /// IR2.4: update preserves id and item_type (immutable fields)
    #[test]
    fn prop_ir2_4_update_preserves_immutable_fields(
        title in "\\PC+",
        new_title in "\\PC+",
        new_data in arb_json(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let data = serde_json::json!({"key": "value"});

            let created = create_item(&pool, &item_type.get_id(), title, data).await.unwrap();
            let updated = update_item(&pool, &created.get_id(), Some(new_title), Some(new_data)).await.unwrap();

            assert_eq!(updated.get_id(), created.get_id());
            assert_eq!(updated.get_item_type(), created.get_item_type());
        });
    }

    /// IR2.5: update advances updated_at
    #[test]
    fn prop_ir2_5_update_advances_updated_at(title in "\\PC+") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let data = serde_json::json!({"key": "value"});

            let created = create_item(&pool, &item_type.get_id(), title.clone(), data).await.unwrap();
            let updated = update_item(&pool, &created.get_id(), Some("New".to_string()), None).await.unwrap();

            assert!(updated.get_updated_at() >= created.get_updated_at(),
                "updated_at should not go backwards: {:?} < {:?}",
                updated.get_updated_at(), created.get_updated_at());
        });
    }

    /// IR2.6: update nonexistent ID returns Err
    #[test]
    fn prop_ir2_6_update_nonexistent_returns_err(id in arb_messy_string()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let result = update_item(&pool, &id, Some("Title".to_string()), None).await;
            assert!(result.is_err(),
                "update_item should return Err for nonexistent id={:?}", id);
        });
    }
}

// ============================================================================
// IR3: List/Filter Properties
// ============================================================================

proptest! {
    /// IR3.1: list_items count equals number of items created
    #[test]
    fn prop_ir3_1_list_count(
        titles in prop::collection::vec("\\PC+", 0..=100)
            .prop_map(dedup_names)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let count = titles.len();

            for title in &titles {
                let data = serde_json::json!({"key": "value"});
                create_item(&pool, &item_type.get_id(), title.clone(), data).await.unwrap();
            }

            let all = list_items(&pool).unwrap();
            assert_eq!(all.len(), count,
                "Expected {} items, got {}", count, all.len());
        });
    }

    /// IR3.2: get_items_by_type returns only items of that type
    #[test]
    fn prop_ir3_2_items_by_type_no_cross_contamination(
        count_a in 0usize..=10,
        count_b in 0usize..=10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let type_a = create_item_type(&pool, "TypeA".to_string()).await.unwrap();
            let type_b = create_item_type(&pool, "TypeB".to_string()).await.unwrap();

            for i in 0..count_a {
                let data = serde_json::json!({"key": "a"});
                create_item(&pool, &type_a.get_id(), format!("A{}", i), data).await.unwrap();
            }
            for i in 0..count_b {
                let data = serde_json::json!({"key": "b"});
                create_item(&pool, &type_b.get_id(), format!("B{}", i), data).await.unwrap();
            }

            let items_a = get_items_by_type(&pool, &type_a.get_id()).unwrap();
            let items_b = get_items_by_type(&pool, &type_b.get_id()).unwrap();

            assert_eq!(items_a.len(), count_a);
            assert_eq!(items_b.len(), count_b);

            // No cross-contamination
            for item in &items_a {
                assert_eq!(item.get_item_type(), type_a.get_id());
            }
            for item in &items_b {
                assert_eq!(item.get_item_type(), type_b.get_id());
            }
        });
    }

    /// IR3.3: get_items_by_type for nonexistent type returns empty vec
    #[test]
    fn prop_ir3_3_items_by_nonexistent_type_empty(id in arb_messy_string()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let items = get_items_by_type(&pool, &id).unwrap();
            assert!(items.is_empty(),
                "get_items_by_type should return empty vec for nonexistent type={:?}", id);
        });
    }

    /// IR3.4: list_items_with_filters(default_query) == list_items
    #[test]
    fn prop_ir3_4_default_filter_equals_list(count in 0usize..=10) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();

            for i in 0..count {
                let data = serde_json::json!({"key": "value"});
                create_item(&pool, &item_type.get_id(), format!("Item{}", i), data).await.unwrap();
            }

            let all = list_items(&pool).unwrap();
            let query = crate::dto::GetQueryDto::default();
            let filtered = list_items_with_filters(&pool, &query).unwrap();

            let all_ids: HashSet<_> = all.iter().map(|i| i.get_id()).collect();
            let filtered_ids: HashSet<_> = filtered.iter().map(|i| i.get_id()).collect();

            assert_eq!(all_ids, filtered_ids,
                "default filter should return same items as list_items");
        });
    }

    /// IR3.5: list_items_with_filters results are a subset of list_items
    #[test]
    fn prop_ir3_5_filtered_subset_of_all(count in 0usize..=10) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let type_a = create_item_type(&pool, "TypeA".to_string()).await.unwrap();
            let type_b = create_item_type(&pool, "TypeB".to_string()).await.unwrap();

            for i in 0..count {
                let data = serde_json::json!({"key": "value"});
                let type_id = if i % 2 == 0 { &type_a } else { &type_b };
                create_item(&pool, &type_id.get_id(), format!("Item{}", i), data).await.unwrap();
            }

            let all_ids: HashSet<_> = list_items(&pool).unwrap().iter().map(|i| i.get_id()).collect();

            let query = crate::dto::GetQueryDtoBuilder::new()
                .item_type_id(type_a.get_id())
                .build();
            let filtered = list_items_with_filters(&pool, &query).unwrap();

            for item in &filtered {
                assert!(all_ids.contains(&item.get_id()),
                    "Filtered item {} should be in list_items", item.get_id());
            }
        });
    }

    /// IR3.6: list_items_with_filters deduplicates: each item appears at most once
    #[test]
    fn prop_ir3_6_filtered_no_duplicates(count in 0usize..=10) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();

            for i in 0..count {
                let data = serde_json::json!({"key": "value"});
                create_item(&pool, &item_type.get_id(), format!("Item{}", i), data).await.unwrap();
            }

            let far_future = chrono::Utc::now() + chrono::Duration::days(365 * 100);
            let query = crate::dto::GetQueryDtoBuilder::new()
                .next_review_before(far_future)
                .build();
            let items = list_items_with_filters(&pool, &query).unwrap();

            let ids: HashSet<_> = items.iter().map(|i| i.get_id()).collect();
            assert_eq!(ids.len(), items.len(),
                "Filtered results should have no duplicate items");
        });
    }
}

// ============================================================================
// IR4: Delete Properties
// ============================================================================

proptest! {
    /// IR4.1: delete→get returns None
    #[test]
    fn prop_ir4_1_delete_then_get_returns_none(title in "\\PC+") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let data = serde_json::json!({"key": "value"});

            let item = create_item(&pool, &item_type.get_id(), title, data).await.unwrap();
            delete_item(&pool, &item.get_id()).await.unwrap();

            let result = get_item(&pool, &item.get_id()).unwrap();
            assert!(result.is_none(),
                "get_item should return None after delete");
        });
    }

    /// IR4.2: delete of nonexistent item does not error
    #[test]
    fn prop_ir4_2_delete_nonexistent_no_error(id in arb_messy_string()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let result = delete_item(&pool, &id).await;
            assert!(result.is_ok(),
                "delete_item should not error for nonexistent id={:?}", id);
        });
    }
}
