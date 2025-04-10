// @generated automatically by Diesel CLI.

diesel::table! {
    cards (id) {
        id -> Text,
        item_id -> Text,
        card_index -> Integer,
        next_review -> Timestamp,
        last_review -> Nullable<Timestamp>,
        scheduler_data -> Nullable<Text>,
        priority -> Float,
        suspended -> Nullable<Timestamp>,
    }
}

diesel::table! {
    item_tags (item_id, tag_id) {
        item_id -> Text,
        tag_id -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    item_types (id) {
        id -> Text,
        name -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    items (id) {
        id -> Text,
        item_type -> Text,
        title -> Text,
        item_data -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    reviews (id) {
        id -> Text,
        card_id -> Text,
        rating -> Integer,
        review_timestamp -> Timestamp,
    }
}

diesel::table! {
    tags (id) {
        id -> Text,
        name -> Text,
        created_at -> Timestamp,
        visible -> Bool,
    }
}

diesel::joinable!(cards -> items (item_id));
diesel::joinable!(item_tags -> items (item_id));
diesel::joinable!(item_tags -> tags (tag_id));
diesel::joinable!(items -> item_types (item_type));
diesel::joinable!(reviews -> cards (card_id));

diesel::allow_tables_to_appear_in_same_query!(
    cards,
    item_tags,
    item_types,
    items,
    reviews,
    tags,
);
