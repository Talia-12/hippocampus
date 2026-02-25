use super::*;
use crate::repo::tests::setup_test_db;
use crate::test_utils::{arb_messy_string, dedup_names};
use proptest::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

// ============================================================================
// TR1: CRUD Roundtrip Properties
// ============================================================================

proptest! {
    /// TR1.1: create→get preserves name and visibility for arbitrary inputs
    #[test]
    fn prop_tr1_1_create_get_preserves_fields(name in "\\PC+", visible in any::<bool>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let created = create_tag(&pool, name.clone(), visible).await.unwrap();
            let retrieved = get_tag(&pool, &created.get_id()).unwrap();

            assert_eq!(retrieved.get_name(), name);
            assert_eq!(retrieved.get_visible(), visible);
            assert_eq!(retrieved.get_id(), created.get_id());
            assert!(Uuid::parse_str(&retrieved.get_id()).is_ok(),
                "ID should be valid UUID, got: {}", retrieved.get_id());
        });
    }

    /// TR1.2: list_tags count equals number of tags created
    #[test]
    fn prop_tr1_2_list_count(
        names in prop::collection::vec("\\PC+", 0..=100)
            .prop_map(dedup_names)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();

            for name in &names {
                create_tag(&pool, name.to_string(), true).await.unwrap();
            }

            let all = list_tags(&pool).unwrap();
            assert_eq!(all.len(), names.len(),
                "Expected {} tags, got {}", names.len(), all.len());
        });
    }

    /// TR1.3: get_tag returns Err for arbitrary nonexistent IDs
    #[test]
    fn prop_tr1_3_get_nonexistent_returns_err(id in arb_messy_string()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let result = get_tag(&pool, &id);
            assert!(result.is_err(),
                "get_tag should return Err for nonexistent id={:?}", id);
        });
    }

    /// TR1.4: bulk create with deduplicated names produces unique, retrievable tags
    #[test]
    fn prop_tr1_4_bulk_create_unique_ids(
        names_and_vis in prop::collection::vec(("\\PC+", any::<bool>()), 0..=100)
            .prop_map(|pairs| {
                let (names, vis): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
                let deduped = dedup_names(names);
                deduped.into_iter().zip(vis.into_iter()).collect::<Vec<_>>()
            })
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let count = names_and_vis.len();

            let mut created = Vec::with_capacity(count);
            for (name, visible) in &names_and_vis {
                created.push(create_tag(&pool, name.to_string(), *visible).await.unwrap());
            }

            // All IDs are unique
            let ids: HashSet<_> = created.iter().map(|t| t.get_id()).collect();
            assert_eq!(ids.len(), count,
                "Expected {} unique IDs, got {}", count, ids.len());

            // Each created tag is retrievable
            for tag in &created {
                let retrieved = get_tag(&pool, &tag.get_id()).unwrap();
                assert_eq!(retrieved.get_id(), tag.get_id());
                assert_eq!(retrieved.get_name(), tag.get_name());
                assert_eq!(retrieved.get_visible(), tag.get_visible());
            }
        });
    }
}

// ============================================================================
// TR2: Association Properties
// ============================================================================

proptest! {
    /// TR2.1: add_tag_to_item is idempotent: adding same tag twice results in exactly 1 association
    #[test]
    fn prop_tr2_1_add_tag_idempotent(repeats in 1usize..=5) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = crate::repo::create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let item = crate::repo::create_item(
                &pool, &item_type.get_id(), "Item".to_string(),
                serde_json::json!({"key": "value"}),
            ).await.unwrap();
            let tag = create_tag(&pool, "Tag".to_string(), true).await.unwrap();

            for _ in 0..repeats {
                add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
            }

            let tags = list_tags_for_item(&pool, &item.get_id()).unwrap();
            assert_eq!(tags.len(), 1,
                "Adding tag {} times should result in exactly 1 association, got {}",
                repeats, tags.len());
        });
    }

    /// TR2.2: add then remove = original state: tag count returns to 0
    #[test]
    fn prop_tr2_2_add_remove_identity(name in "\\PC+", visible in any::<bool>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = crate::repo::create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let item = crate::repo::create_item(
                &pool, &item_type.get_id(), "Item".to_string(),
                serde_json::json!({"key": "value"}),
            ).await.unwrap();
            let tag = create_tag(&pool, name, visible).await.unwrap();

            // Before: no tags
            let before = list_tags_for_item(&pool, &item.get_id()).unwrap();
            assert_eq!(before.len(), 0);

            // Add then remove
            add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
            remove_tag_from_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();

            // After: back to no tags
            let after = list_tags_for_item(&pool, &item.get_id()).unwrap();
            assert_eq!(after.len(), 0,
                "After add+remove, tag count should be 0, got {}", after.len());
        });
    }

    /// TR2.3: tag isolation: adding a tag to item A does not affect item B's tags
    #[test]
    fn prop_tr2_3_tag_isolation(
        count_a in 0usize..=5,
        count_b in 0usize..=5,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = crate::repo::create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let item_a = crate::repo::create_item(
                &pool, &item_type.get_id(), "ItemA".to_string(),
                serde_json::json!({"key": "a"}),
            ).await.unwrap();
            let item_b = crate::repo::create_item(
                &pool, &item_type.get_id(), "ItemB".to_string(),
                serde_json::json!({"key": "b"}),
            ).await.unwrap();

            // Create distinct tags for each item
            for i in 0..count_a {
                let tag = create_tag(&pool, format!("TagA{}", i), true).await.unwrap();
                add_tag_to_item(&pool, &tag.get_id(), &item_a.get_id()).await.unwrap();
            }
            for i in 0..count_b {
                let tag = create_tag(&pool, format!("TagB{}", i), true).await.unwrap();
                add_tag_to_item(&pool, &tag.get_id(), &item_b.get_id()).await.unwrap();
            }

            let tags_a = list_tags_for_item(&pool, &item_a.get_id()).unwrap();
            let tags_b = list_tags_for_item(&pool, &item_b.get_id()).unwrap();

            assert_eq!(tags_a.len(), count_a,
                "Item A should have {} tags, got {}", count_a, tags_a.len());
            assert_eq!(tags_b.len(), count_b,
                "Item B should have {} tags, got {}", count_b, tags_b.len());

            // No overlap
            let ids_a: HashSet<_> = tags_a.iter().map(|t| t.get_id()).collect();
            let ids_b: HashSet<_> = tags_b.iter().map(|t| t.get_id()).collect();
            assert!(ids_a.is_disjoint(&ids_b),
                "Tags on item A and B should not overlap");
        });
    }

    /// TR2.4: remove_tag_from_item for nonexistent association returns Err
    #[test]
    fn prop_tr2_4_remove_nonexistent_association_returns_err(
        name in "\\PC+",
        visible in any::<bool>(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = crate::repo::create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let item = crate::repo::create_item(
                &pool, &item_type.get_id(), "Item".to_string(),
                serde_json::json!({"key": "value"}),
            ).await.unwrap();
            let tag = create_tag(&pool, name, visible).await.unwrap();

            // Don't add the tag — removing should error
            let result = remove_tag_from_item(&pool, &tag.get_id(), &item.get_id()).await;
            assert!(result.is_err(),
                "remove_tag_from_item should return Err for nonexistent association");
        });
    }

    /// TR2.5: adding N distinct tags → list_tags_for_item has N entries
    #[test]
    fn prop_tr2_5_multiple_tags_on_one_item(count in 0usize..=20) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = crate::repo::create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let item = crate::repo::create_item(
                &pool, &item_type.get_id(), "Item".to_string(),
                serde_json::json!({"key": "value"}),
            ).await.unwrap();

            let mut tag_ids = Vec::new();
            for i in 0..count {
                let tag = create_tag(&pool, format!("Tag{}", i), true).await.unwrap();
                add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
                tag_ids.push(tag.get_id());
            }

            let tags = list_tags_for_item(&pool, &item.get_id()).unwrap();
            assert_eq!(tags.len(), count,
                "Expected {} tags on item, got {}", count, tags.len());

            // All expected tags present
            let retrieved_ids: HashSet<_> = tags.iter().map(|t| t.get_id()).collect();
            for id in &tag_ids {
                assert!(retrieved_ids.contains(id),
                    "Tag {} should be on item", id);
            }
        });
    }
}

// ============================================================================
// TR3: Card-Item Tag Consistency
// ============================================================================

proptest! {
    /// TR3.1: list_tags_for_card returns same set as list_tags_for_item for that card's item
    #[test]
    fn prop_tr3_1_card_item_tag_consistency(count in 0usize..=10) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = crate::repo::create_item_type(&pool, "TestType".to_string()).await.unwrap();
            let item = crate::repo::create_item(
                &pool, &item_type.get_id(), "Item".to_string(),
                serde_json::json!({"key": "value"}),
            ).await.unwrap();

            // Get the card for this item
            let cards = crate::repo::get_cards_for_item(&pool, &item.get_id()).unwrap();
            assert!(!cards.is_empty(), "Item should have at least one card");
            let card = &cards[0];

            // Add some tags to the item
            for i in 0..count {
                let tag = create_tag(&pool, format!("Tag{}", i), true).await.unwrap();
                add_tag_to_item(&pool, &tag.get_id(), &item.get_id()).await.unwrap();
            }

            // Compare tags from both paths
            let item_tags = list_tags_for_item(&pool, &item.get_id()).unwrap();
            let card_tags = list_tags_for_card(&pool, &card.get_id()).unwrap();

            let item_tag_ids: HashSet<_> = item_tags.iter().map(|t| t.get_id()).collect();
            let card_tag_ids: HashSet<_> = card_tags.iter().map(|t| t.get_id()).collect();

            assert_eq!(item_tag_ids, card_tag_ids,
                "list_tags_for_card and list_tags_for_item should return the same tag set");
        });
    }
}
