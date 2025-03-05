// @generated automatically by Diesel CLI.

diesel::table! {
    items (id) {
        id -> Nullable<Text>,
        title -> Text,
        next_review -> Nullable<Timestamp>,
        last_review -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    reviews (id) {
        id -> Nullable<Text>,
        item_id -> Text,
        rating -> Integer,
        review_timestamp -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    items,
    reviews,
);
