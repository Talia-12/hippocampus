use super::*;
use crate::test_utils::{arb_optional_datetime_utc, arb_messy_string, arb_priority, arb_suspended_filter, arb_json};
use proptest::prelude::*;

/// Generates an arbitrary GetQueryDto via the builder
fn arb_get_query_dto() -> impl Strategy<Value = GetQueryDto> {
    (
        prop::option::of(arb_messy_string()),
        prop::collection::vec(arb_messy_string(), 0..5),
        arb_optional_datetime_utc(),
        arb_optional_datetime_utc(),
        arb_suspended_filter(),
        arb_optional_datetime_utc(),
        arb_optional_datetime_utc(),
        prop::option::of(any::<bool>()),
    )
        .prop_map(
            |(item_type_id, tag_ids, next_review_before, last_review_after, suspended_filter, suspended_after, suspended_before, split_priority)| {
                let mut builder = GetQueryDtoBuilder::new()
                    .tag_ids(tag_ids)
                    .suspended_filter(suspended_filter);

                if let Some(id) = item_type_id {
                    builder = builder.item_type_id(id);
                }
                if let Some(dt) = next_review_before {
                    builder = builder.next_review_before(dt);
                }
                if let Some(dt) = last_review_after {
                    builder = builder.last_review_after(dt);
                }
                if let Some(dt) = suspended_after {
                    builder = builder.suspended_after(dt);
                }
                if let Some(dt) = suspended_before {
                    builder = builder.suspended_before(dt);
                }
                if let Some(sp) = split_priority {
                    builder = builder.split_priority(sp);
                }

                builder.build()
            },
        )
}

proptest! {
    /// D1.1: Builder produces DTO with matching fields
    #[test]
    fn prop_d1_1_builder_roundtrip(
        item_type_id in prop::option::of(arb_messy_string()),
        tag_ids in prop::collection::vec(arb_messy_string(), 0..5),
        next_review_before in arb_optional_datetime_utc(),
        last_review_after in arb_optional_datetime_utc(),
        suspended_filter in arb_suspended_filter(),
        split_priority in prop::option::of(any::<bool>()),
    ) {
        let mut builder = GetQueryDtoBuilder::new()
            .tag_ids(tag_ids.clone())
            .suspended_filter(suspended_filter);

        if let Some(ref id) = item_type_id {
            builder = builder.item_type_id(id.clone());
        }
        if let Some(dt) = next_review_before {
            builder = builder.next_review_before(dt);
        }
        if let Some(dt) = last_review_after {
            builder = builder.last_review_after(dt);
        }
        if let Some(sp) = split_priority {
            builder = builder.split_priority(sp);
        }

        let dto = builder.build();

        prop_assert_eq!(&dto.item_type_id, &item_type_id);
        prop_assert_eq!(&dto.tag_ids, &tag_ids);
        prop_assert_eq!(dto.next_review_before, next_review_before);
        prop_assert_eq!(dto.last_review_after, last_review_after);
        prop_assert_eq!(dto.suspended_filter, suspended_filter);
        prop_assert_eq!(dto.split_priority, split_priority);
    }

    /// D1.2: add_tag_id appends to existing list
    #[test]
    fn prop_d1_2_builder_add_tag_id_appends(
        initial_tags in prop::collection::vec(arb_messy_string(), 0..5),
        new_tag in arb_messy_string(),
    ) {
        let builder = GetQueryDtoBuilder::new()
            .tag_ids(initial_tags.clone())
            .add_tag_id(new_tag.clone());

        let dto = builder.build();

        prop_assert_eq!(dto.tag_ids.len(), initial_tags.len() + 1);
        prop_assert_eq!(dto.tag_ids.last().unwrap(), &new_tag);
        for (i, tag) in initial_tags.iter().enumerate() {
            prop_assert_eq!(&dto.tag_ids[i], tag);
        }
    }

    /// D1.3: Display impl doesn't panic for arbitrary DTOs
    #[test]
    fn prop_d1_3_display_does_not_panic(dto in arb_get_query_dto()) {
        let _display = format!("{}", dto);
        // If we get here without panic, the test passes
    }

    /// D1.4: CreateItemDto JSON serde roundtrip
    #[test]
    fn prop_d1_4_create_item_dto_serde_roundtrip(
        item_type_id in arb_messy_string(),
        title in arb_messy_string(),
        item_data in arb_json(),
        priority in arb_priority(),
    ) {
        let dto = CreateItemDto {
            item_type_id: item_type_id.clone(),
            title: title.clone(),
            item_data: item_data.clone(),
            priority,
        };
        let json_str = serde_json::to_string(&dto).unwrap();
        let deserialized: CreateItemDto = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(&deserialized.item_type_id, &item_type_id);
        prop_assert_eq!(&deserialized.title, &title);
        prop_assert!(crate::test_utils::json_approx_eq(&deserialized.item_data, &item_data));
        prop_assert!((deserialized.priority - priority).abs() < 1e-3);
    }

    /// D1.5: CreateReviewDto JSON serde roundtrip
    #[test]
    fn prop_d1_5_create_review_dto_serde_roundtrip(
        card_id in arb_messy_string(),
        rating in any::<i32>(),
    ) {
        let dto = CreateReviewDto {
            card_id: card_id.clone(),
            rating,
        };
        let json_str = serde_json::to_string(&dto).unwrap();
        let deserialized: CreateReviewDto = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(&deserialized.card_id, &card_id);
        prop_assert_eq!(deserialized.rating, rating);
    }

    /// D1.6: SuspendedFilter all variants roundtrip
    #[test]
    fn prop_d1_6_suspended_filter_serde_roundtrip(filter in arb_suspended_filter()) {
        let json_str = serde_json::to_string(&filter).unwrap();
        let deserialized: SuspendedFilter = serde_json::from_str(&json_str).unwrap();
        prop_assert_eq!(filter, deserialized);
    }
}
