use super::*;
use crate::repo::tests::setup_test_db;
use crate::repo::create_item_type;
use crate::test_utils::{arb_item_params, arb_relation_type, create_items};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// BFS from `start` following adjacency list edges.
///
/// Does NOT include `start` in the returned set unless it is reachable
/// via a cycle. This means: a cycle exists iff `start ∈ bfs_reachable(adj, start)`.
fn bfs_reachable(adj: &HashMap<String, Vec<String>>, start: &str) -> HashSet<String> {
	let mut visited = HashSet::new();
	let mut queue = VecDeque::new();
	for neighbor in adj.get(start).unwrap_or(&vec![]) {
		queue.push_back(neighbor.clone());
	}
	while let Some(node) = queue.pop_front() {
		if visited.insert(node.clone()) {
			for neighbor in adj.get(&node).unwrap_or(&vec![]) {
				queue.push_back(neighbor.clone());
			}
		}
	}
	visited
}

/// Builds a parent→children adjacency list from a list of ItemRelation
fn build_adj(relations: &[crate::models::ItemRelation]) -> HashMap<String, Vec<String>> {
	let mut adj: HashMap<String, Vec<String>> = HashMap::new();
	for rel in relations {
		adj.entry(rel.get_parent_item_id())
			.or_default()
			.push(rel.get_child_item_id());
	}
	adj
}

/// Builds a child→parents adjacency list (reverse direction) from a list of ItemRelation
fn build_reverse_adj(relations: &[crate::models::ItemRelation]) -> HashMap<String, Vec<String>> {
	let mut adj: HashMap<String, Vec<String>> = HashMap::new();
	for rel in relations {
		adj.entry(rel.get_child_item_id())
			.or_default()
			.push(rel.get_parent_item_id());
	}
	adj
}

/// Helper: create items, attempt random edges, return (items, pool)
/// Edges that would create cycles or duplicates are silently skipped.
async fn setup_random_graph(
	pool: &DbPool,
	item_params: Vec<(String, serde_json::Value)>,
	edges: &[(usize, usize, String)],
) -> Vec<crate::models::Item> {
	let item_type = create_item_type(pool, "Test Type".to_string(), "fsrs".to_string())
		.await
		.unwrap();
	let items = create_items(pool, &item_type.get_id(), item_params).await;
	let n = items.len();
	if n < 2 {
		return items;
	}

	for (parent_idx, child_idx, rel_type) in edges {
		let p = &items[parent_idx % n];
		let c = &items[child_idx % n];
		// Skip self-loops at the index level too
		if p.get_id() == c.get_id() {
			continue;
		}
		// Ignore cycle and duplicate errors
		let _ = create_item_relation(pool, &p.get_id(), &c.get_id(), rel_type).await;
	}

	items
}

// ============================================================================
// IR1: CRUD Properties
// ============================================================================

proptest! {
	/// IR1.1: create→get round-trip preserves parent_id, child_id, relation_type
	#[test]
	fn prop_ir1_1_create_roundtrip(
		item_params in arb_item_params(10).prop_filter("need >=2 items", |v| v.len() >= 2),
		rel_type in arb_relation_type(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
			let items = create_items(&pool, &item_type.get_id(), item_params).await;

			let parent = &items[0];
			let child = &items[1];

			let created = create_item_relation(&pool, &parent.get_id(), &child.get_id(), &rel_type).await.unwrap();

			assert_eq!(created.get_parent_item_id(), parent.get_id());
			assert_eq!(created.get_child_item_id(), child.get_id());
			assert_eq!(created.get_relation_type(), rel_type);

			let retrieved = get_item_relation(&pool, &parent.get_id(), &child.get_id()).unwrap().unwrap();
			assert_eq!(retrieved.get_parent_item_id(), parent.get_id());
			assert_eq!(retrieved.get_child_item_id(), child.get_id());
			assert_eq!(retrieved.get_relation_type(), rel_type);
		});
	}

	/// IR1.2: creating the same relation twice is idempotent (second attempt errors, original remains)
	#[test]
	fn prop_ir1_2_create_idempotent(
		item_params in arb_item_params(10).prop_filter("need >=2 items", |v| v.len() >= 2),
		rel_type in arb_relation_type(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
			let items = create_items(&pool, &item_type.get_id(), item_params).await;

			let parent = &items[0];
			let child = &items[1];

			create_item_relation(&pool, &parent.get_id(), &child.get_id(), &rel_type).await.unwrap();

			// Second create may error with UNIQUE constraint — that's expected
			let _ = create_item_relation(&pool, &parent.get_id(), &child.get_id(), &rel_type).await;

			// Original relation should still be retrievable
			let retrieved = get_item_relation(&pool, &parent.get_id(), &child.get_id()).unwrap().unwrap();
			assert_eq!(retrieved.get_parent_item_id(), parent.get_id());
			assert_eq!(retrieved.get_child_item_id(), child.get_id());
			assert_eq!(retrieved.get_relation_type(), rel_type);

			// Exactly one relation between this pair
			let all = list_item_relations(&pool, Some(&parent.get_id()), Some(&child.get_id()), None).unwrap();
			assert_eq!(all.len(), 1);
		});
	}

	/// IR1.3: delete is the inverse of create
	#[test]
	fn prop_ir1_3_delete_inverse_of_create(
		item_params in arb_item_params(10).prop_filter("need >=2 items", |v| v.len() >= 2),
		rel_type in arb_relation_type(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
			let items = create_items(&pool, &item_type.get_id(), item_params).await;

			let parent = &items[0];
			let child = &items[1];

			create_item_relation(&pool, &parent.get_id(), &child.get_id(), &rel_type).await.unwrap();
			delete_item_relation(&pool, &parent.get_id(), &child.get_id()).await.unwrap();

			let result = get_item_relation(&pool, &parent.get_id(), &child.get_id()).unwrap();
			assert!(result.is_none(), "Relation should be gone after delete");
		});
	}

	/// IR1.4: delete nonexistent relation returns an error
	#[test]
	fn prop_ir1_4_delete_nonexistent_errors(
		item_params in arb_item_params(10).prop_filter("need >=2 items", |v| v.len() >= 2),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
			let items = create_items(&pool, &item_type.get_id(), item_params).await;

			let result = delete_item_relation(&pool, &items[0].get_id(), &items[1].get_id()).await;
			assert!(result.is_err(), "Deleting nonexistent relation should error");
		});
	}

	/// IR1.5: get nonexistent relation returns None
	#[test]
	fn prop_ir1_5_get_nonexistent_returns_none(
		item_params in arb_item_params(10).prop_filter("need >=2 items", |v| v.len() >= 2),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let item_type = create_item_type(&pool, "Test Type".to_string(), "fsrs".to_string()).await.unwrap();
			let items = create_items(&pool, &item_type.get_id(), item_params).await;

			let result = get_item_relation(&pool, &items[0].get_id(), &items[1].get_id()).unwrap();
			assert!(result.is_none(), "get_item_relation should return None for nonexistent relation");
		});
	}
}

// ============================================================================
// IR2: List / Filter Properties
// ============================================================================

proptest! {
	/// IR2.1: list with no filters returns all successfully inserted relations
	#[test]
	fn prop_ir2_1_list_all_returns_all(
		item_params in arb_item_params(100).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..100, 0usize..100, arb_relation_type()), 0..=50),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;
			let n = items.len();

			// Count how many edges we expect succeeded
			let mut success_count = 0usize;
			let mut seen_pairs = HashSet::new();
			for (parent_idx, child_idx, _) in &edges {
				let pi = parent_idx % n;
				let ci = child_idx % n;
				let pid = items[pi].get_id();
				let cid = items[ci].get_id();
				if pid == cid { continue; }
				let pair = (pid.clone(), cid.clone());
				if seen_pairs.contains(&pair) { continue; }
				// Check if it would have been a cycle
				if get_item_relation(&pool, &pid, &cid).unwrap().is_some() {
					if seen_pairs.insert(pair) {
						success_count += 1;
					}
				}
			}

			let all = list_item_relations(&pool, None, None, None).unwrap();
			assert_eq!(all.len(), success_count,
				"list_item_relations(None, None, None) should return all {} relations", success_count);
		});
	}

	/// IR2.2: parent filter returns only matching relations and is a subset of unfiltered
	#[test]
	fn prop_ir2_2_parent_filter_correct(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=30),
		filter_idx in 0usize..20,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;
			let n = items.len();
			let filter_id = items[filter_idx % n].get_id();

			let all = list_item_relations(&pool, None, None, None).unwrap();
			let filtered = list_item_relations(&pool, Some(&filter_id), None, None).unwrap();

			// Every result has the correct parent
			for rel in &filtered {
				assert_eq!(rel.get_parent_item_id(), filter_id,
					"Filtered result should have parent_item_id = {}", filter_id);
			}

			// Filtered is a subset of unfiltered
			let all_pairs: HashSet<_> = all.iter()
				.map(|r| (r.get_parent_item_id(), r.get_child_item_id()))
				.collect();
			for rel in &filtered {
				assert!(all_pairs.contains(&(rel.get_parent_item_id(), rel.get_child_item_id())),
					"Filtered result should be subset of unfiltered list");
			}
		});
	}

	/// IR2.3: child filter returns only matching relations and is a subset of unfiltered
	#[test]
	fn prop_ir2_3_child_filter_correct(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=30),
		filter_idx in 0usize..20,
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;
			let n = items.len();
			let filter_id = items[filter_idx % n].get_id();

			let all = list_item_relations(&pool, None, None, None).unwrap();
			let filtered = list_item_relations(&pool, None, Some(&filter_id), None).unwrap();

			for rel in &filtered {
				assert_eq!(rel.get_child_item_id(), filter_id,
					"Filtered result should have child_item_id = {}", filter_id);
			}

			let all_pairs: HashSet<_> = all.iter()
				.map(|r| (r.get_parent_item_id(), r.get_child_item_id()))
				.collect();
			for rel in &filtered {
				assert!(all_pairs.contains(&(rel.get_parent_item_id(), rel.get_child_item_id())),
					"Filtered result should be subset of unfiltered list");
			}
		});
	}

	/// IR2.4: relation type filter returns only matching relations and is a subset of unfiltered
	#[test]
	fn prop_ir2_4_relation_type_filter_correct(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=30),
		filter_type in arb_relation_type(),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let _items = setup_random_graph(&pool, item_params, &edges).await;

			let all = list_item_relations(&pool, None, None, None).unwrap();
			let filtered = list_item_relations(&pool, None, None, Some(&filter_type)).unwrap();

			for rel in &filtered {
				assert_eq!(rel.get_relation_type(), filter_type,
					"Filtered result should have relation_type = {}", filter_type);
			}

			let all_pairs: HashSet<_> = all.iter()
				.map(|r| (r.get_parent_item_id(), r.get_child_item_id()))
				.collect();
			for rel in &filtered {
				assert!(all_pairs.contains(&(rel.get_parent_item_id(), rel.get_child_item_id())),
					"Filtered result should be subset of unfiltered list");
			}
		});
	}
}

// ============================================================================
// IR3: DAG Invariant Properties
// ============================================================================

proptest! {
	/// IR3.1: no cycles after random insertions — verified by BFS
	#[test]
	fn prop_ir3_1_no_cycles_bfs(
		item_params in arb_item_params(100).prop_filter("need >=5 items", |v| v.len() >= 5),
		edges in prop::collection::vec((0usize..100, 0usize..100, arb_relation_type()), 0..=250),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;

			let all = list_item_relations(&pool, None, None, None).unwrap();
			let adj = build_adj(&all);

			// For every item, BFS should NOT find the item itself (no cycle)
			for item in &items {
				let reachable = bfs_reachable(&adj, &item.get_id());
				assert!(!reachable.contains(&item.get_id()),
					"Cycle detected: item {} is reachable from itself", item.get_id());
			}
		});
	}

	/// IR3.2: `would_create_cycle` agrees with BFS oracle
	#[test]
	fn prop_ir3_2_cycle_detection_agrees_with_bfs(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=50),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;

			let all = list_item_relations(&pool, None, None, None).unwrap();
			let adj = build_adj(&all);

			// Check every pair of items
			for i in &items {
				for j in &items {
					let db_says_cycle = would_create_cycle(&pool, &i.get_id(), &j.get_id()).unwrap();

					// BFS oracle: adding edge i→j creates a cycle iff
					// j can already reach i (j is ancestor of i), or i == j
					let oracle_cycle = if i.get_id() == j.get_id() {
						true
					} else {
						// If j can reach i via children, then adding i→j closes a cycle
						bfs_reachable(&adj, &j.get_id()).contains(&i.get_id())
					};

					assert_eq!(db_says_cycle, oracle_cycle,
						"would_create_cycle({}, {}) = {} but BFS oracle says {}",
						i.get_id(), j.get_id(), db_says_cycle, oracle_cycle);
				}
			}
		});
	}
}

// ============================================================================
// IR4: Traversal Properties
// ============================================================================

proptest! {
	/// IR4.1: get_children_of matches direct edges from list_item_relations
	#[test]
	fn prop_ir4_1_children_matches_list(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=30),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;

			for item in &items {
				let children: HashSet<_> = get_children_of(&pool, &item.get_id())
					.unwrap()
					.into_iter()
					.collect();

				let from_list: HashSet<_> = list_item_relations(&pool, Some(&item.get_id()), None, None)
					.unwrap()
					.into_iter()
					.map(|r| r.get_child_item_id())
					.collect();

				assert_eq!(children, from_list,
					"get_children_of({}) should match list_item_relations filter",
					item.get_id());
			}
		});
	}

	/// IR4.2: get_parents_of matches direct edges from list_item_relations
	#[test]
	fn prop_ir4_2_parents_matches_list(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=30),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;

			for item in &items {
				let parents: HashSet<_> = get_parents_of(&pool, &item.get_id())
					.unwrap()
					.into_iter()
					.collect();

				let from_list: HashSet<_> = list_item_relations(&pool, None, Some(&item.get_id()), None)
					.unwrap()
					.into_iter()
					.map(|r| r.get_parent_item_id())
					.collect();

				assert_eq!(parents, from_list,
					"get_parents_of({}) should match list_item_relations filter",
					item.get_id());
			}
		});
	}

	/// IR4.3: get_all_descendants is the transitive closure downward (BFS verification)
	#[test]
	fn prop_ir4_3_descendants_transitive_closure(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=30),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;

			let all = list_item_relations(&pool, None, None, None).unwrap();
			let adj = build_adj(&all);

			for item in &items {
				let desc_edges = get_all_descendants(&pool, &item.get_id()).unwrap();

				// Collect reachable item IDs from the descendant edges
				let mut desc_ids: HashSet<String> = HashSet::new();
				for (p, c, _) in &desc_edges {
					desc_ids.insert(p.clone());
					desc_ids.insert(c.clone());
				}
				// Remove the root itself — it appears as parent in first-level edges
				// but BFS doesn't include start
				// Actually: collect only child_ids reachable through the edge set
				let desc_child_ids: HashSet<String> = desc_edges.iter()
					.map(|(_, c, _)| c.clone())
					.collect();

				let bfs_ids = bfs_reachable(&adj, &item.get_id());

				assert_eq!(desc_child_ids, bfs_ids,
					"get_all_descendants({}) child IDs should match BFS reachable set",
					item.get_id());
			}
		});
	}

	/// IR4.4: get_all_ancestors is the transitive closure upward (BFS verification)
	#[test]
	fn prop_ir4_4_ancestors_transitive_closure(
		item_params in arb_item_params(20).prop_filter("need >=3 items", |v| v.len() >= 3),
		edges in prop::collection::vec((0usize..20, 0usize..20, arb_relation_type()), 0..=30),
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();
		rt.block_on(async {
			let pool = setup_test_db();
			let items = setup_random_graph(&pool, item_params, &edges).await;

			let all = list_item_relations(&pool, None, None, None).unwrap();
			let reverse_adj = build_reverse_adj(&all);

			for item in &items {
				let anc_edges = get_all_ancestors(&pool, &item.get_id()).unwrap();

				// Collect parent IDs from ancestor edges
				let anc_parent_ids: HashSet<String> = anc_edges.iter()
					.map(|(p, _, _)| p.clone())
					.collect();

				let bfs_ids = bfs_reachable(&reverse_adj, &item.get_id());

				assert_eq!(anc_parent_ids, bfs_ids,
					"get_all_ancestors({}) parent IDs should match BFS reachable set (upward)",
					item.get_id());
			}
		});
	}
}
