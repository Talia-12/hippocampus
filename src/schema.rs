// @generated automatically by Diesel CLI.

diesel::table! {
    items (id) {
        id -> Text,
        title -> Text,
        next_review -> Nullable<Timestamp>,
        last_review -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
