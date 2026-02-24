use super::*;
use crate::repo::tests::setup_test_db;
use crate::repo::{create_item, create_item_type, create_tag, add_tag_to_item};
use crate::test_utils::{
    arb_card_mutations, arb_datetime_utc, arb_invalid_priority, arb_optional_datetime_utc,
    arb_priority, arb_priority_offset, arb_suspended_filter, CardMutations,
};
use crate::GetQueryDto;
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;


// ============================================================================
// Oracle function for filter correctness property tests
// ============================================================================

/// Pure-Rust oracle that replicates the semantics of list_cards_with_filters.
///
/// Applies all filter predicates with AND composition, matching the SQL
/// implementation's behavior including NULL semantics.
fn oracle_filter(
    all_cards: &[Card],
    query: &GetQueryDto,
    item_type_map: &HashMap<String, String>,
    item_tags_map: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    all_cards
        .iter()
        .filter(|card| {
            // item_type_id filter
            if let Some(ref type_id) = query.item_type_id {
                if item_type_map.get(&card.get_item_id()).map(|t| t.as_str())
                    != Some(type_id.as_str())
                {
                    return false;
                }
            }

            // next_review_before: next_review is NOT NULL in schema
            if let Some(cutoff) = query.next_review_before {
                if !(card.get_next_review() < cutoff) {
                    return false;
                }
            }

            // last_review_after: last_review IS nullable
            // SQL: .gt(date).and(.is_not_null()) — NULL > x is NULL (falsy)
            if let Some(cutoff) = query.last_review_after {
                if !card.get_last_review().map_or(false, |lr| lr > cutoff) {
                    return false;
                }
            }

            // suspended_filter
            match query.suspended_filter {
                SuspendedFilter::Exclude => {
                    if card.get_suspended().is_some() {
                        return false;
                    }
                }
                SuspendedFilter::Only => {
                    if card.get_suspended().is_none() {
                        return false;
                    }
                }
                SuspendedFilter::Include => {}
            }

            // suspended_before: SQL NULL < x is NULL (falsy)
            if let Some(cutoff) = query.suspended_before {
                if !card.get_suspended().map_or(false, |s| s < cutoff) {
                    return false;
                }
            }

            // suspended_after: SQL NULL > x is NULL (falsy)
            if let Some(cutoff) = query.suspended_after {
                if !card.get_suspended().map_or(false, |s| s > cutoff) {
                    return false;
                }
            }

            // tag_ids filter (AND semantics)
            if !query.tag_ids.is_empty() {
                let item_tags = item_tags_map
                    .get(&card.get_item_id())
                    .cloned()
                    .unwrap_or_default();
                if !query.tag_ids.iter().all(|tid| item_tags.contains(tid)) {
                    return false;
                }
            }

            true
        })
        .map(|c| c.get_id())
        .collect()
}

/// Helper: apply CardMutations to a Card and persist via update_card
async fn apply_mutations_to_card(
    pool: &std::sync::Arc<crate::db::DbPool>,
    card: &mut Card,
    mutations: &CardMutations,
) {
    card.set_next_review(mutations.next_review);
    card.set_last_review(mutations.last_review);
    card.set_priority(mutations.priority);
    card.set_suspended(mutations.suspended);
    update_card(pool, card).await.unwrap();
}

/// Helper: collect card IDs as a sorted Vec for set comparison
fn sorted_ids(cards: &[Card]) -> Vec<String> {
    let mut ids: Vec<String> = cards.iter().map(|c| c.get_id()).collect();
    ids.sort();
    ids
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}


// ============================================================================
// T1: CRUD Round-Trip Property Tests
// ============================================================================

proptest! {
    /// T1.1: create_card + get_card preserves all fields
    #[test]
    fn prop_t1_1_create_read_identity(
        card_index in 2..100i32,  // start at 2 to avoid conflict with auto-created cards (0, 1)
        priority in arb_priority(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
            let item = create_item(
                &pool, &item_type.get_id(), "Test".to_string(),
                json!({"front": "F", "back": "B"}),
            ).await.unwrap();

            let card = create_card(&pool, &item.get_id(), card_index, priority).await.unwrap();
            let retrieved = get_card(&pool, &card.get_id()).unwrap().unwrap();

            prop_assert_eq!(retrieved.get_id(), card.get_id());
            prop_assert_eq!(retrieved.get_item_id(), card.get_item_id());
            prop_assert_eq!(retrieved.get_card_index(), card.get_card_index());
            prop_assert!((retrieved.get_priority() - card.get_priority()).abs() < 1e-6);
            prop_assert_eq!(retrieved.get_last_review(), None);
            prop_assert_eq!(retrieved.get_suspended(), None);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T1.2: update_card + get_card preserves all mutable fields
    #[test]
    fn prop_t1_2_update_read_identity(mutations in arb_card_mutations()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
            let item = create_item(
                &pool, &item_type.get_id(), "Test".to_string(),
                json!({"front": "F", "back": "B"}),
            ).await.unwrap();
            let mut card = get_cards_for_item(&pool, &item.get_id()).unwrap().remove(0);

            apply_mutations_to_card(&pool, &mut card, &mutations).await;
            let retrieved = get_card(&pool, &card.get_id()).unwrap().unwrap();

            prop_assert_eq!(retrieved.get_next_review(), mutations.next_review);
            prop_assert_eq!(retrieved.get_last_review(), mutations.last_review);
            prop_assert!((retrieved.get_priority() - mutations.priority).abs() < 1e-6);
            prop_assert_eq!(retrieved.get_suspended(), mutations.suspended);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T1.3: update_card_priority with valid value succeeds and roundtrips
    #[test]
    fn prop_t1_3_priority_valid(priority in arb_priority()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
            let item = create_item(
                &pool, &item_type.get_id(), "Test".to_string(),
                json!({"front": "F", "back": "B"}),
            ).await.unwrap();
            let card = get_cards_for_item(&pool, &item.get_id()).unwrap().remove(0);

            let updated = update_card_priority(&pool, &card.get_id(), priority).await.unwrap();
            prop_assert!((updated.get_priority() - priority).abs() < 1e-6);

            let retrieved = get_card(&pool, &card.get_id()).unwrap().unwrap();
            prop_assert!((retrieved.get_priority() - priority).abs() < 1e-6);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T1.4: update_card_priority with invalid value fails and leaves card unchanged
    #[test]
    fn prop_t1_4_priority_invalid(priority in arb_invalid_priority()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
            let item = create_item(
                &pool, &item_type.get_id(), "Test".to_string(),
                json!({"front": "F", "back": "B"}),
            ).await.unwrap();
            let card = get_cards_for_item(&pool, &item.get_id()).unwrap().remove(0);
            let original_priority = card.get_priority();

            let result = update_card_priority(&pool, &card.get_id(), priority).await;
            prop_assert!(result.is_err());

            let retrieved = get_card(&pool, &card.get_id()).unwrap().unwrap();
            prop_assert!((retrieved.get_priority() - original_priority).abs() < 1e-6);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T1.5: set_card_suspended toggle correctness
    #[test]
    fn prop_t1_5_suspension_toggle(initial_suspended in any::<bool>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
            let item = create_item(
                &pool, &item_type.get_id(), "Test".to_string(),
                json!({"front": "F", "back": "B"}),
            ).await.unwrap();
            let card = get_cards_for_item(&pool, &item.get_id()).unwrap().remove(0);

            // Set to initial state
            set_card_suspended(&pool, &card.get_id(), initial_suspended).await.unwrap();
            let after_set = get_card(&pool, &card.get_id()).unwrap().unwrap();
            prop_assert_eq!(after_set.get_suspended().is_some(), initial_suspended);

            // Toggle
            set_card_suspended(&pool, &card.get_id(), !initial_suspended).await.unwrap();
            let after_toggle = get_card(&pool, &card.get_id()).unwrap().unwrap();
            prop_assert_eq!(after_toggle.get_suspended().is_some(), !initial_suspended);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T1.6: set_card_suspended is idempotent
    #[test]
    fn prop_t1_6_suspension_idempotent(target in any::<bool>()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();
            let item = create_item(
                &pool, &item_type.get_id(), "Test".to_string(),
                json!({"front": "F", "back": "B"}),
            ).await.unwrap();
            let card = get_cards_for_item(&pool, &item.get_id()).unwrap().remove(0);

            // Apply twice
            set_card_suspended(&pool, &card.get_id(), target).await.unwrap();
            let after_first = get_card(&pool, &card.get_id()).unwrap().unwrap();

            set_card_suspended(&pool, &card.get_id(), target).await.unwrap();
            let after_second = get_card(&pool, &card.get_id()).unwrap().unwrap();

            prop_assert_eq!(after_first.get_suspended().is_some(), target);
            prop_assert_eq!(after_second.get_suspended().is_some(), target);
            // If suspended, the timestamp should not change on second call
            prop_assert_eq!(after_first.get_suspended(), after_second.get_suspended());
            Ok::<_, TestCaseError>(())
        })?;
    }
}


// ============================================================================
// Helper: set up a universe of cards with varied states for filter testing
// ============================================================================

/// Creates N items (each getting 2 cards for "Test" type) and applies random mutations.
/// Returns (all_cards, item_type_map, items).
async fn setup_filter_universe(
    pool: &std::sync::Arc<crate::db::DbPool>,
    n_items: usize,
    card_mutations: &[CardMutations],
) -> (Vec<Card>, HashMap<String, String>, Vec<crate::models::Item>) {
    let type1 = create_item_type(pool, "Test Type A".to_string()).await.unwrap();
    let type2 = create_item_type(pool, "Test Type B".to_string()).await.unwrap();
    let types = [type1.get_id(), type2.get_id()];

    let mut items = vec![];
    let mut item_type_map = HashMap::new();

    for i in 0..n_items {
        let type_id = &types[i % 2];
        let item = create_item(
            pool, type_id, format!("Item{}", i),
            json!({"front": "F", "back": "B"}),
        ).await.unwrap();
        item_type_map.insert(item.get_id(), type_id.clone());
        items.push(item);
    }

    // Get all cards, apply mutations
    let all_cards_initial = list_all_cards(pool).unwrap();
    for (idx, card_ref) in all_cards_initial.iter().enumerate() {
        let m = &card_mutations[idx % card_mutations.len()];
        let mut card = card_ref.clone();
        apply_mutations_to_card(pool, &mut card, m).await;
    }

    let all_cards = list_all_cards(pool).unwrap();
    (all_cards, item_type_map, items)
}


// ============================================================================
// T2: Filter Correctness Property Tests (Decomposed)
// ============================================================================

proptest! {
    /// T2.1: item_type_id filter — soundness & completeness
    #[test]
    fn prop_t2_1_filter_item_type(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
        filter_first_type in any::<bool>(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, item_type_map, _items) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            // Pick which type to filter by
            let target_type = if filter_first_type {
                item_type_map.values().next().unwrap().clone()
            } else {
                item_type_map.values().last().unwrap().clone()
            };

            let query = GetQueryDto {
                item_type_id: Some(target_type.clone()),
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &HashMap::new());

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            // Also verify soundness directly: every result has the right type
            for c in &sql_result {
                prop_assert_eq!(
                    item_type_map.get(&c.get_item_id()).unwrap(),
                    &target_type
                );
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.2: next_review_before filter — soundness & completeness
    #[test]
    fn prop_t2_2_filter_next_review_before(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
        cutoff in arb_datetime_utc(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, item_type_map, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                next_review_before: Some(cutoff),
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &HashMap::new());

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            // Soundness: every result has next_review < cutoff
            for c in &sql_result {
                prop_assert!(c.get_next_review() < cutoff);
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.3: last_review_after filter — soundness & completeness (NULL edge case)
    #[test]
    fn prop_t2_3_filter_last_review_after(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
        cutoff in arb_datetime_utc(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, item_type_map, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                last_review_after: Some(cutoff),
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &HashMap::new());

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            // Soundness: every result has last_review > cutoff (not None)
            for c in &sql_result {
                let lr = c.get_last_review();
                prop_assert!(lr.is_some(), "Card with None last_review in result");
                prop_assert!(lr.unwrap() > cutoff);
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.4: suspended_filter=Exclude — only non-suspended cards
    #[test]
    fn prop_t2_4_filter_suspended_exclude(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, item_type_map, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Exclude,
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &HashMap::new());

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            for c in &sql_result {
                prop_assert!(c.get_suspended().is_none());
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.5: suspended_filter=Only — only suspended cards
    #[test]
    fn prop_t2_5_filter_suspended_only(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, item_type_map, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Only,
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &HashMap::new());

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            for c in &sql_result {
                prop_assert!(c.get_suspended().is_some());
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.6: suspended_filter=Include — returns all cards
    #[test]
    fn prop_t2_6_filter_suspended_include(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, _, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            prop_assert_eq!(sorted_ids(&sql_result), sorted_ids(&all_cards));
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.7: suspended_before filter (NULL edge: non-suspended cards excluded)
    #[test]
    fn prop_t2_7_filter_suspended_before(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
        cutoff in arb_datetime_utc(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, item_type_map, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                suspended_before: Some(cutoff),
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &HashMap::new());

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            // Soundness: every result has suspended < cutoff (and is_some)
            for c in &sql_result {
                let s = c.get_suspended();
                prop_assert!(s.is_some(), "Non-suspended card in suspended_before result");
                prop_assert!(s.unwrap() < cutoff);
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.8: suspended_after filter (NULL edge: non-suspended cards excluded)
    #[test]
    fn prop_t2_8_filter_suspended_after(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
        cutoff in arb_datetime_utc(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, item_type_map, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                suspended_after: Some(cutoff),
                ..Default::default()
            };

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &HashMap::new());

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            for c in &sql_result {
                let s = c.get_suspended();
                prop_assert!(s.is_some(), "Non-suspended card in suspended_after result");
                prop_assert!(s.unwrap() > cutoff);
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.9: tag_ids filter (AND semantics)
    #[test]
    fn prop_t2_9_filter_tags(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
        // Bitmask for which items get which tags (up to 3 items x 2 tags)
        tag_assign in prop::collection::vec(prop::collection::vec(any::<bool>(), 2..=2), 3..=3),
        // Which tags to filter by
        filter_tag0 in any::<bool>(),
        filter_tag1 in any::<bool>(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let (_, item_type_map, items) =
                setup_filter_universe(&pool, 3, &card_mutations).await;

            // Create 2 tags
            let tag0 = create_tag(&pool, "TagA".to_string(), true).await.unwrap();
            let tag1 = create_tag(&pool, "TagB".to_string(), true).await.unwrap();
            let tag_ids = [tag0.get_id(), tag1.get_id()];

            // Assign tags to items based on bitmask
            let mut item_tags_map: HashMap<String, Vec<String>> = HashMap::new();
            for (i, item) in items.iter().enumerate() {
                for (j, tid) in tag_ids.iter().enumerate() {
                    if tag_assign[i][j] {
                        add_tag_to_item(&pool, tid, &item.get_id()).await.unwrap();
                        item_tags_map.entry(item.get_id()).or_default().push(tid.clone());
                    }
                }
            }

            // Build filter tag list
            let mut query_tags = vec![];
            if filter_tag0 { query_tags.push(tag_ids[0].clone()); }
            if filter_tag1 { query_tags.push(tag_ids[1].clone()); }

            let query = GetQueryDto {
                tag_ids: query_tags,
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };

            // Re-read all cards (tags don't change card data but we need fresh state)
            let all_cards = list_all_cards(&pool).unwrap();

            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &item_tags_map);

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.10: FULL COMPOSITION — all filters combined, oracle comparison
    #[test]
    fn prop_t2_10_full_oracle(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..100),
        tag_assign in prop::collection::vec(prop::collection::vec(any::<bool>(), 2..=2), 3..=3),
        use_type_filter in any::<bool>(),
        filter_tag0 in any::<bool>(),
        filter_tag1 in any::<bool>(),
        query_nrb in arb_optional_datetime_utc(),
        query_lra in arb_optional_datetime_utc(),
        query_sf in arb_suspended_filter(),
        query_sa in arb_optional_datetime_utc(),
        query_sb in arb_optional_datetime_utc(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let (_, item_type_map, items) =
                setup_filter_universe(&pool, 3, &card_mutations).await;

            // Create and assign tags
            let tag0 = create_tag(&pool, "TagA".to_string(), true).await.unwrap();
            let tag1 = create_tag(&pool, "TagB".to_string(), true).await.unwrap();
            let tag_ids_all = [tag0.get_id(), tag1.get_id()];

            let mut item_tags_map: HashMap<String, Vec<String>> = HashMap::new();
            for (i, item) in items.iter().enumerate() {
                for (j, tid) in tag_ids_all.iter().enumerate() {
                    if tag_assign[i % tag_assign.len()][j] {
                        add_tag_to_item(&pool, tid, &item.get_id()).await.unwrap();
                        item_tags_map.entry(item.get_id()).or_default().push(tid.clone());
                    }
                }
            }

            // Build the random query
            let query_type_id = if use_type_filter {
                Some(item_type_map.values().next().unwrap().clone())
            } else {
                None
            };
            let mut query_tags = vec![];
            if filter_tag0 { query_tags.push(tag_ids_all[0].clone()); }
            if filter_tag1 { query_tags.push(tag_ids_all[1].clone()); }

            let query = GetQueryDto {
                item_type_id: query_type_id,
                tag_ids: query_tags,
                next_review_before: query_nrb,
                last_review_after: query_lra,
                suspended_filter: query_sf,
                suspended_after: query_sa,
                suspended_before: query_sb,
                split_priority: None,
            };

            let all_cards = list_all_cards(&pool).unwrap();
            let sql_result = list_cards_with_filters(&pool, &query).unwrap();
            let oracle_result = oracle_filter(&all_cards, &query, &item_type_map, &item_tags_map);

            prop_assert_eq!(sorted_ids(&sql_result), sorted(oracle_result));
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.11: Nonexistent type/tag IDs return empty
    #[test]
    fn prop_t2_11_nonexistent_ids(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..1000),
        use_bad_type in any::<bool>(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            setup_filter_universe(&pool, 2, &card_mutations).await;

            let query = if use_bad_type {
                GetQueryDto {
                    item_type_id: Some("nonexistent-type-id".to_string()),
                    suspended_filter: SuspendedFilter::Include,
                    ..Default::default()
                }
            } else {
                GetQueryDto {
                    tag_ids: vec!["nonexistent-tag-id".to_string()],
                    suspended_filter: SuspendedFilter::Include,
                    ..Default::default()
                }
            };

            let result = list_cards_with_filters(&pool, &query).unwrap();
            prop_assert_eq!(result.len(), 0);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T2.12: Identity query (Include, no filters) returns all cards
    #[test]
    fn prop_t2_12_identity_query(
        card_mutations in prop::collection::vec(arb_card_mutations(), 2..8),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let n_items = (card_mutations.len() / 2).max(2);
            let (all_cards, _, _) =
                setup_filter_universe(&pool, n_items, &card_mutations).await;

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };

            let result = list_cards_with_filters(&pool, &query).unwrap();
            prop_assert_eq!(sorted_ids(&result), sorted_ids(&all_cards));
            Ok::<_, TestCaseError>(())
        })?;
    }
}


// ============================================================================
// T3: Structural Invariant Property Tests
// ============================================================================

proptest! {
    /// T3.1: Card sets for different items are disjoint
    #[test]
    fn prop_t3_1_cards_disjoint(
        n_items in 2usize..1000,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();

            let mut items = vec![];
            for i in 0..n_items {
                let item = create_item(
                    &pool, &item_type.get_id(), format!("Item{}", i),
                    json!({"front": "F", "back": "B"}),
                ).await.unwrap();
                items.push(item);
            }

            // Collect card ID sets per item
            let mut all_card_ids = std::collections::HashSet::new();
            let mut total_count = 0usize;
            for item in &items {
                let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
                for c in &cards {
                    all_card_ids.insert(c.get_id());
                }
                total_count += cards.len();
            }

            // If disjoint, the set size equals the total count
            prop_assert_eq!(all_card_ids.len(), total_count);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T3.2: Union of per-item cards equals list_all_cards
    #[test]
    fn prop_t3_2_cards_union_complete(
        n_items in 1usize..1000,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();

            let mut items = vec![];
            for i in 0..n_items {
                let item = create_item(
                    &pool, &item_type.get_id(), format!("Item{}", i),
                    json!({"front": "F", "back": "B"}),
                ).await.unwrap();
                items.push(item);
            }

            let all_cards = list_all_cards(&pool).unwrap();
            let mut union_ids: Vec<String> = vec![];
            for item in &items {
                let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
                union_ids.extend(cards.iter().map(|c| c.get_id()));
            }

            union_ids.sort();
            let mut all_ids = sorted_ids(&all_cards);
            all_ids.sort();

            prop_assert_eq!(union_ids, all_ids);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T3.3: get_cards_for_item returns cards ordered by card_index ASC
    #[test]
    fn prop_t3_3_cards_ordered(
        n_items in 1usize..1000,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Test Type 1".to_string()).await.unwrap();

            for i in 0..n_items {
                let item = create_item(
                    &pool, &item_type.get_id(), format!("Item{}", i),
                    json!({"front": "F", "back": "B"}),
                ).await.unwrap();

                let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
                // Verify card_index is strictly ascending
                for w in cards.windows(2) {
                    prop_assert!(w[0].get_card_index() < w[1].get_card_index(),
                        "Cards not ordered: {} >= {}", w[0].get_card_index(), w[1].get_card_index());
                }
            }
            Ok::<_, TestCaseError>(())
        })?;
    }
}


// ============================================================================
// Helper: create N basic items and return their first cards
// ============================================================================

async fn create_n_cards(
    pool: &std::sync::Arc<crate::db::DbPool>,
    n: usize,
) -> Vec<Card> {
    let item_type = create_item_type(pool, "Basic".to_string()).await.unwrap();
    let mut cards = Vec::new();
    for i in 0..n {
        let item = create_item(
            pool, &item_type.get_id(), format!("Item{}", i),
            json!({"front": "F", "back": "B"}),
        ).await.unwrap();
        let item_cards = get_cards_for_item(pool, &item.get_id()).unwrap();
        cards.push(item_cards.into_iter().next().unwrap());
    }
    cards
}


// ============================================================================
// T4: Sort Position Property Tests (DB Operations & Ordering)
// ============================================================================

proptest! {
    /// T4.1: After move_card_to_top(c), card c is first in list_cards_with_filters
    #[test]
    fn prop_t4_1_move_to_top_is_first(n in 2usize..20) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            // Give all cards sort positions via move_card_to_top
            for c in &cards {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            // Move a card in the middle to top
            let target = &cards[n / 2];
            move_card_to_top(&pool, &target.get_id()).await.unwrap();

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };
            let result = list_cards_with_filters(&pool, &query).unwrap();
            prop_assert_eq!(result[0].get_id(), target.get_id());
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.2: After move_card_to_top, pairwise ordering of other positioned cards is unchanged
    #[test]
    fn prop_t4_2_move_to_top_preserves_others_order(n in 3usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            // Give all cards sort positions
            for c in &cards {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };
            let before = list_cards_with_filters(&pool, &query).unwrap();
            let target_id = cards[n / 2].get_id();

            // Record ordering of others before
            let others_before: Vec<String> = before.iter()
                .filter(|c| c.get_id() != target_id)
                .map(|c| c.get_id())
                .collect();

            move_card_to_top(&pool, &target_id).await.unwrap();

            let after = list_cards_with_filters(&pool, &query).unwrap();
            let others_after: Vec<String> = after.iter()
                .filter(|c| c.get_id() != target_id)
                .map(|c| c.get_id())
                .collect();

            prop_assert_eq!(others_before, others_after);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.3: Assigned sort_position < all other existing sort_positions
    #[test]
    fn prop_t4_3_move_to_top_position_strictly_less(n in 2usize..20) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            for c in &cards {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            let target = &cards[n / 2];
            let updated = move_card_to_top(&pool, &target.get_id()).await.unwrap();
            let new_pos = updated.get_sort_position().unwrap();

            let all = list_all_cards(&pool).unwrap();
            for c in &all {
                if c.get_id() != target.get_id() {
                    if let Some(pos) = c.get_sort_position() {
                        prop_assert!(new_pos < pos,
                            "top card pos {} not < other pos {}", new_pos, pos);
                    }
                }
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.4: When no cards have sort_positions, move_card_to_top assigns 0.0
    #[test]
    fn prop_t4_4_move_to_top_first_card_gets_zero(n in 1usize..10) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            let updated = move_card_to_top(&pool, &cards[0].get_id()).await.unwrap();
            prop_assert_eq!(updated.get_sort_position(), Some(0.0));
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.5: After move_card_relative(c, t, before=true), c.sort_position < t.sort_position
    #[test]
    fn prop_t4_5_move_relative_before_ordering(n in 3usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            // Give all cards sort positions
            for c in cards.iter().rev() {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            let card_id = &cards[0].get_id();
            let target_id = &cards[n - 1].get_id();

            let moved = move_card_relative(&pool, card_id, target_id, true).await.unwrap();
            let target = get_card(&pool, target_id).unwrap().unwrap();

            prop_assert!(moved.get_sort_position().unwrap() < target.get_sort_position().unwrap());
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.6: After move_card_relative(c, t, before=false), c.sort_position > t.sort_position
    #[test]
    fn prop_t4_6_move_relative_after_ordering(n in 3usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            for c in cards.iter().rev() {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            let card_id = &cards[n - 1].get_id();
            let target_id = &cards[0].get_id();

            let moved = move_card_relative(&pool, card_id, target_id, false).await.unwrap();
            let target = get_card(&pool, target_id).unwrap().unwrap();

            prop_assert!(moved.get_sort_position().unwrap() > target.get_sort_position().unwrap());
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.7: After move_card_relative, pairwise ordering of uninvolved cards is unchanged
    #[test]
    fn prop_t4_7_move_relative_preserves_others_order(n in 4usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            for c in cards.iter().rev() {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };
            let before = list_cards_with_filters(&pool, &query).unwrap();

            let mover_id = cards[0].get_id();
            let target_id = cards[n - 1].get_id();

            let others_before: Vec<String> = before.iter()
                .filter(|c| c.get_id() != mover_id)
                .map(|c| c.get_id())
                .collect();

            move_card_relative(&pool, &mover_id, &target_id, false).await.unwrap();

            let after = list_cards_with_filters(&pool, &query).unwrap();
            let others_after: Vec<String> = after.iter()
                .filter(|c| c.get_id() != mover_id)
                .map(|c| c.get_id())
                .collect();

            prop_assert_eq!(others_before, others_after);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.8: After clear_sort_positions, all cards have sort_position == None
    #[test]
    fn prop_t4_8_clear_all_sort_positions_nulls_all(n in 1usize..20) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            for c in &cards {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            clear_sort_positions(&pool).await.unwrap();

            let all = list_all_cards(&pool).unwrap();
            for c in &all {
                prop_assert_eq!(c.get_sort_position(), None,
                    "card {} still has sort_position", c.get_id());
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.9: clear_card_sort_position(c) sets only c to None; others unchanged
    #[test]
    fn prop_t4_9_clear_single_sort_position_only_target(n in 2usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            for c in &cards {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            // Record positions before
            let before: Vec<(String, Option<f32>)> = list_all_cards(&pool).unwrap()
                .iter().map(|c| (c.get_id(), c.get_sort_position())).collect();

            let target_id = cards[n / 2].get_id();
            clear_card_sort_position(&pool, &target_id).await.unwrap();

            let after = list_all_cards(&pool).unwrap();
            for c in &after {
                if c.get_id() == target_id {
                    prop_assert_eq!(c.get_sort_position(), None);
                } else {
                    let orig = before.iter().find(|(id, _)| *id == c.get_id()).unwrap().1;
                    prop_assert_eq!(c.get_sort_position(), orig,
                        "card {} sort_position changed", c.get_id());
                }
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.10: All cards with sort_position appear before all cards without in list results
    #[test]
    fn prop_t4_10_nulls_last_ordering(n in 2usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            // Give half the cards sort positions
            for c in cards.iter().take(n / 2) {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };
            let result = list_cards_with_filters(&pool, &query).unwrap();

            let mut seen_null = false;
            for c in &result {
                if c.get_sort_position().is_none() {
                    seen_null = true;
                } else {
                    prop_assert!(!seen_null,
                        "positioned card {} appears after a null-positioned card", c.get_id());
                }
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.11: A card with sort_position appears before a card without sort_position
    #[test]
    fn prop_t4_11_sort_position_trumps_priority(
        low_prio in 0.0f32..0.1f32,
        high_prio in 0.9f32..1.0f32,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let item_type = create_item_type(&pool, "Basic".to_string()).await.unwrap();

            // Card A: low priority, will get sort_position
            let item_a = create_item(&pool, &item_type.get_id(), "A".to_string(),
                json!({"front": "F", "back": "B"})).await.unwrap();
            let card_a = get_cards_for_item(&pool, &item_a.get_id()).unwrap().remove(0);
            update_card_priority(&pool, &card_a.get_id(), low_prio).await.unwrap();
            move_card_to_top(&pool, &card_a.get_id()).await.unwrap();

            // Card B: high priority, no sort_position
            let item_b = create_item(&pool, &item_type.get_id(), "B".to_string(),
                json!({"front": "F", "back": "B"})).await.unwrap();
            let card_b = get_cards_for_item(&pool, &item_b.get_id()).unwrap().remove(0);
            update_card_priority(&pool, &card_b.get_id(), high_prio).await.unwrap();

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };
            let result = list_cards_with_filters(&pool, &query).unwrap();

            let pos_a = result.iter().position(|c| c.get_id() == card_a.get_id()).unwrap();
            let pos_b = result.iter().position(|c| c.get_id() == card_b.get_id()).unwrap();
            prop_assert!(pos_a < pos_b,
                "positioned card (pos {}) should appear before unpositioned card (pos {})", pos_a, pos_b);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T4.12: Among cards with sort_position=None, ordering is by (priority + priority_offset) DESC
    #[test]
    fn prop_t4_12_effective_priority_ordering_among_nulls(n in 2usize..10) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            // Set distinct priorities with offsets, no sort_positions
            for (i, c) in cards.iter().enumerate() {
                let priority = (i as f32 + 1.0) / (n as f32 + 1.0);
                update_card_priority(&pool, &c.get_id(), priority).await.unwrap();
                // Set a small offset that doesn't change relative ordering
                let mut card = get_card(&pool, &c.get_id()).unwrap().unwrap();
                card.set_priority_offset(0.001 * i as f32);
                update_card(&pool, &card).await.unwrap();
            }

            let query = GetQueryDto {
                suspended_filter: SuspendedFilter::Include,
                ..Default::default()
            };
            let result = list_cards_with_filters(&pool, &query).unwrap();

            // All have None sort_position, so should be ordered by effective priority DESC
            for w in result.windows(2) {
                let eff_a = w[0].get_priority() + w[0].get_priority_offset();
                let eff_b = w[1].get_priority() + w[1].get_priority_offset();
                prop_assert!(eff_a >= eff_b,
                    "effective priority ordering violated: {} < {}", eff_a, eff_b);
            }
            Ok::<_, TestCaseError>(())
        })?;
    }
}


// ============================================================================
// T4 Error Cases (unit-style)
// ============================================================================

#[tokio::test]
async fn test_t4_e1_move_to_top_nonexistent_card() {
    let pool = setup_test_db();
    let result = move_card_to_top(&pool, "nonexistent-id").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_t4_e2_move_relative_nonexistent_card() {
    let pool = setup_test_db();
    let cards = create_n_cards(&pool, 2).await;
    move_card_to_top(&pool, &cards[1].get_id()).await.unwrap();
    let result = move_card_relative(&pool, "nonexistent-id", &cards[1].get_id(), true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_t4_e3_move_relative_nonexistent_target() {
    let pool = setup_test_db();
    let cards = create_n_cards(&pool, 1).await;
    let result = move_card_relative(&pool, &cards[0].get_id(), "nonexistent-id", true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_t4_e4_move_relative_target_no_position() {
    let pool = setup_test_db();
    let cards = create_n_cards(&pool, 2).await;
    // Target has no sort_position (default None)
    let result = move_card_relative(&pool, &cards[0].get_id(), &cards[1].get_id(), true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_t4_e5_clear_card_sort_position_nonexistent() {
    let pool = setup_test_db();
    let result = clear_card_sort_position(&pool, "nonexistent-id").await;
    assert!(result.is_err());
}


// ============================================================================
// T5: Priority Offset Property Tests (DB Operations)
// ============================================================================

proptest! {
    /// T5.1: After regenerate_priority_offsets, all cards have offset in [-0.05, +0.05]
    #[test]
    fn prop_t5_1_regenerate_offsets_in_range(n in 1usize..20) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let _cards = create_n_cards(&pool, n).await;

            regenerate_priority_offsets(&pool).await.unwrap();

            let all = list_all_cards(&pool).unwrap();
            for c in &all {
                let off = c.get_priority_offset();
                prop_assert!(off >= -0.05 && off <= 0.05,
                    "offset {} out of range for card {}", off, c.get_id());
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T5.2: After regeneration, metadata last_offset_date == today's date
    #[test]
    fn prop_t5_2_regenerate_updates_metadata_date(n in 1usize..5) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let _cards = create_n_cards(&pool, n).await;

            regenerate_priority_offsets(&pool).await.unwrap();

            let conn = &mut pool.get().unwrap();
            let last_date: String = metadata::table
                .find("last_offset_date")
                .select(metadata::value)
                .first::<String>(conn)
                .unwrap();

            let today = chrono::Utc::now().date_naive().to_string();
            prop_assert_eq!(last_date, today);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T5.3: With 20+ cards, not all offsets are identical
    #[test]
    fn prop_t5_3_regenerate_not_all_same(_seed in 0u32..100u32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let _cards = create_n_cards(&pool, 25).await;

            regenerate_priority_offsets(&pool).await.unwrap();

            let all = list_all_cards(&pool).unwrap();
            let offsets: std::collections::HashSet<u32> = all.iter()
                .map(|c| c.get_priority_offset().to_bits())
                .collect();

            prop_assert!(offsets.len() > 1,
                "all 25 cards got identical offsets — highly improbable");
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T5.4: Calling ensure_offsets_current when offsets were just regenerated is a noop
    #[test]
    fn prop_t5_4_ensure_current_is_noop_when_current(n in 1usize..10) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let _cards = create_n_cards(&pool, n).await;

            regenerate_priority_offsets(&pool).await.unwrap();

            let before: Vec<(String, f32)> = list_all_cards(&pool).unwrap()
                .iter().map(|c| (c.get_id(), c.get_priority_offset())).collect();

            ensure_offsets_current(&pool).await.unwrap();

            let after: Vec<(String, f32)> = list_all_cards(&pool).unwrap()
                .iter().map(|c| (c.get_id(), c.get_priority_offset())).collect();

            prop_assert_eq!(before, after);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T5.5: update_card_priority(c, p) → c.priority_offset == 0.0 and c.priority == p
    #[test]
    fn prop_t5_5_update_priority_resets_offset(
        priority in arb_priority(),
        offset in arb_priority_offset(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, 1).await;
            let card_id = cards[0].get_id();

            // Set a non-zero offset first
            let mut card = get_card(&pool, &card_id).unwrap().unwrap();
            card.set_priority_offset(offset);
            update_card(&pool, &card).await.unwrap();

            // Now update priority — should reset offset to 0.0
            let updated = update_card_priority(&pool, &card_id, priority).await.unwrap();

            prop_assert!((updated.get_priority() - priority).abs() < 1e-6);
            prop_assert_eq!(updated.get_priority_offset(), 0.0);
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T5.6: regenerate_priority_offsets does not change any card's base priority
    #[test]
    fn prop_t5_6_regenerate_preserves_base_priority(n in 1usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            // Set distinct priorities
            for (i, c) in cards.iter().enumerate() {
                let p = (i as f32 + 1.0) / (n as f32 + 1.0);
                update_card_priority(&pool, &c.get_id(), p).await.unwrap();
            }

            let before: Vec<(String, f32)> = list_all_cards(&pool).unwrap()
                .iter().map(|c| (c.get_id(), c.get_priority())).collect();

            regenerate_priority_offsets(&pool).await.unwrap();

            let after: Vec<(String, f32)> = list_all_cards(&pool).unwrap()
                .iter().map(|c| (c.get_id(), c.get_priority())).collect();

            for (id, before_p) in &before {
                let after_p = after.iter().find(|(aid, _)| aid == id).unwrap().1;
                prop_assert!((before_p - after_p).abs() < 1e-6,
                    "base priority changed for card {}", id);
            }
            Ok::<_, TestCaseError>(())
        })?;
    }

    /// T5.7: regenerate_priority_offsets does not change any card's sort_position
    #[test]
    fn prop_t5_7_regenerate_preserves_sort_position(n in 1usize..15) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = setup_test_db();
            let cards = create_n_cards(&pool, n).await;

            // Give some cards sort positions
            for c in cards.iter().take(n / 2 + 1) {
                move_card_to_top(&pool, &c.get_id()).await.unwrap();
            }

            let before: Vec<(String, Option<f32>)> = list_all_cards(&pool).unwrap()
                .iter().map(|c| (c.get_id(), c.get_sort_position())).collect();

            regenerate_priority_offsets(&pool).await.unwrap();

            let after: Vec<(String, Option<f32>)> = list_all_cards(&pool).unwrap()
                .iter().map(|c| (c.get_id(), c.get_sort_position())).collect();

            for (id, before_pos) in &before {
                let after_pos = &after.iter().find(|(aid, _)| aid == id).unwrap().1;
                prop_assert_eq!(before_pos, after_pos,
                    "sort_position changed for card {}", id);
            }
            Ok::<_, TestCaseError>(())
        })?;
    }
}
