use super::*;

#[test]
fn test_new_sets_fields() {
    let item_id = "item-123".to_string();
    let tag_id = "tag-456".to_string();
    let item_tag = ItemTag::new(item_id.clone(), tag_id.clone());

    assert_eq!(item_tag.get_item_id(), item_id);
    assert_eq!(item_tag.get_tag_id(), tag_id);
}

#[test]
fn test_created_at_is_recent() {
    let before = Utc::now();
    let item_tag = ItemTag::new("item".to_string(), "tag".to_string());
    let after = Utc::now();

    let created_at = item_tag.get_created_at();
    assert!(created_at >= before, "created_at should be >= test start time");
    assert!(created_at <= after, "created_at should be <= test end time");
}
