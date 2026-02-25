use super::*;
use crate::repo::tests::setup_test_db;
use crate::test_utils::arb_messy_string;
use proptest::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

/// Deduplicates a vec of strings by appending increasing indices to duplicates.
///
/// For example: ["cat", "cat", "cat1"] → ["cat", "cat1", "cat2"]
/// Handles cascading collisions (e.g. appending "1" creates a new collision).
fn dedup_names(names: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(names.len());

    for name in names {
        if seen.insert(name.clone()) {
            result.push(name);
        } else {
            let mut idx = 1u64;
            loop {
                let candidate = format!("{}{}", name, idx);
                if seen.insert(candidate.clone()) {
                    result.push(candidate);
                    break;
                }
                idx += 1;
            }
        }
    }

    result
}

// ============================================================================
// ITR1: CRUD Roundtrip Properties
// ============================================================================

proptest! {
    /// ITR1.1: create→get preserves name for arbitrary strings
    #[test]
    fn prop_itr1_1_create_get_preserves_name(name in "\\PC+") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let created = create_item_type(&pool, name.clone()).await.unwrap();
            let retrieved = get_item_type(&pool, &created.get_id()).unwrap().unwrap();

            assert_eq!(retrieved.get_name(), name);
            assert_eq!(retrieved.get_id(), created.get_id());
        });
    }

    /// ITR1.2: create produces valid UUID
    #[test]
    fn prop_itr1_2_create_produces_valid_uuid(name in "\\PC+") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, name).await.unwrap();
            assert!(Uuid::parse_str(&item_type.get_id()).is_ok(),
                "ID should be valid UUID, got: {}", item_type.get_id());
        });
    }

    /// ITR1.3: list_item_types count equals number of types created
    #[test]
    fn prop_itr1_3_list_count(
        names in prop::collection::vec("\\PC+", 0..=100)
            .prop_map(dedup_names)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();

            for name in &names {
                create_item_type(&pool, name.to_string()).await.unwrap();
            }

            let all = list_item_types(&pool).unwrap();
            assert_eq!(all.len(), names.len(),
                "Expected {} item types, got {}", names.len(), all.len());
        });
    }

    /// ITR1.4: get_item_type returns None for arbitrary nonexistent IDs
    #[test]
    fn prop_itr1_4_get_nonexistent_returns_none(id in arb_messy_string()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let result = get_item_type(&pool, &id).unwrap();
            assert!(result.is_none(),
                "get_item_type should return None for nonexistent id={:?}", id);
        });
    }

    /// ITR1.5: bulk create with deduplicated names produces unique, retrievable types
    #[test]
    fn prop_itr1_5_bulk_create_unique_ids(
        names in prop::collection::vec("\\PC+", 0..=100)
            .prop_map(dedup_names)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let count = names.len();

            let mut created = Vec::with_capacity(count);
            for name in &names {
                created.push(create_item_type(&pool, name.to_string()).await.unwrap());
            }

            // All IDs are unique
            let ids: HashSet<_> = created.iter().map(|it| it.get_id()).collect();
            assert_eq!(ids.len(), count,
                "Expected {} unique IDs, got {}", count, ids.len());

            // Each created item type is retrievable
            for item_type in &created {
                let retrieved = get_item_type(&pool, &item_type.get_id())
                    .unwrap()
                    .expect("created item type should be retrievable");
                assert_eq!(retrieved.get_id(), item_type.get_id());
                assert_eq!(retrieved.get_name(), item_type.get_name());
            }
        });
    }
}
