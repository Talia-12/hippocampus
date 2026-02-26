use super::*;
use serde_json::json;

#[test]
fn test_default_priority() {
    assert!((default_priority() - 0.5).abs() < f32::EPSILON);
}

#[test]
fn test_get_query_dto_default() {
    let dto = GetQueryDto::default();
    assert!(dto.item_type_id.is_none());
    assert!(dto.tag_ids.is_empty());
    assert!(dto.next_review_before.is_none());
    assert!(dto.last_review_after.is_none());
    assert_eq!(dto.suspended_filter, SuspendedFilter::Exclude);
    assert!(dto.suspended_after.is_none());
    assert!(dto.suspended_before.is_none());
    assert!(dto.split_priority.is_none());
}

#[test]
fn test_sort_position_action_serde_top() {
    let action = SortPositionAction::Top;
    let json_str = serde_json::to_string(&action).unwrap();
    let deserialized: SortPositionAction = serde_json::from_str(&json_str).unwrap();
    assert!(matches!(deserialized, SortPositionAction::Top));
}

#[test]
fn test_sort_position_action_serde_before() {
    let action = SortPositionAction::Before {
        card_id: "card-123".to_string(),
    };
    let json_str = serde_json::to_string(&action).unwrap();
    let deserialized: SortPositionAction = serde_json::from_str(&json_str).unwrap();
    match deserialized {
        SortPositionAction::Before { card_id } => assert_eq!(card_id, "card-123"),
        _ => panic!("Expected Before variant"),
    }
}

#[test]
fn test_sort_position_action_serde_after() {
    let action = SortPositionAction::After {
        card_id: "card-456".to_string(),
    };
    let json_str = serde_json::to_string(&action).unwrap();
    let deserialized: SortPositionAction = serde_json::from_str(&json_str).unwrap();
    match deserialized {
        SortPositionAction::After { card_id } => assert_eq!(card_id, "card-456"),
        _ => panic!("Expected After variant"),
    }
}

#[test]
fn test_get_query_dto_display_empty() {
    let dto = GetQueryDto::default();
    let display = format!("{}", dto);
    assert!(display.contains("item_type_id: None"));
    assert!(display.contains("tag_ids: []"));
    assert!(display.contains("next_review_before: None"));
    assert!(display.contains("last_review_after: None"));
}

#[test]
fn test_get_query_dto_display_full() {
    use chrono::TimeZone;
    let dto = GetQueryDto {
        item_type_id: Some("type-1".to_string()),
        tag_ids: vec!["tag-a".to_string(), "tag-b".to_string()],
        next_review_before: Some(Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap()),
        last_review_after: Some(Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
        suspended_filter: SuspendedFilter::Include,
        suspended_after: None,
        suspended_before: None,
        split_priority: Some(true),
    };
    let display = format!("{}", dto);
    assert!(display.contains("item_type_id: type-1"));
    assert!(display.contains("tag-a"));
    assert!(display.contains("tag-b"));
    assert!(display.contains("next_review_before: 2025-06-15"));
    assert!(display.contains("last_review_after: 2025-01-01"));
}

#[test]
fn test_suspended_filter_serde_roundtrip() {
    for filter in &[
        SuspendedFilter::Include,
        SuspendedFilter::Exclude,
        SuspendedFilter::Only,
    ] {
        let json_str = serde_json::to_string(filter).unwrap();
        let deserialized: SuspendedFilter = serde_json::from_str(&json_str).unwrap();
        assert_eq!(*filter, deserialized);
    }
}

#[test]
fn test_create_item_dto_serde_roundtrip() {
    let dto = CreateItemDto {
        item_type_id: "type-1".to_string(),
        title: "Test Item".to_string(),
        item_data: json!({"key": "value"}),
        priority: 0.7,
    };
    let json_str = serde_json::to_string(&dto).unwrap();
    let deserialized: CreateItemDto = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.item_type_id, "type-1");
    assert_eq!(deserialized.title, "Test Item");
    assert_eq!(deserialized.item_data, json!({"key": "value"}));
    assert!((deserialized.priority - 0.7).abs() < f32::EPSILON);
}

#[test]
fn test_create_item_dto_default_priority() {
    // When priority is not specified in JSON, it should default to 0.5
    let json_str = r#"{"item_type_id":"t1","title":"Test","item_data":null}"#;
    let dto: CreateItemDto = serde_json::from_str(json_str).unwrap();
    assert!((dto.priority - 0.5).abs() < f32::EPSILON);
}
