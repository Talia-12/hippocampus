// @generated automatically by Diesel CLI.

diesel::table! {
	card_fetched_events (item_type_id, order_index) {
		item_type_id -> Text,
		order_index -> Integer,
		function_name -> Text,
	}
}

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
		sort_position -> Float,
		priority_offset -> Float,
		card_data -> Nullable<Text>,
		updated_at -> Timestamp,
		cache_updated_at -> Nullable<Timestamp>,
	}
}

diesel::table! {
	item_relations (parent_item_id, child_item_id) {
		parent_item_id -> Text,
		child_item_id -> Text,
		relation_type -> Text,
		created_at -> Timestamp,
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
		review_function -> Text,
		updated_at -> Timestamp,
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
	metadata (key) {
		key -> Text,
		value -> Text,
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

diesel::joinable!(card_fetched_events -> item_types (item_type_id));
diesel::joinable!(cards -> items (item_id));
diesel::joinable!(item_tags -> items (item_id));
diesel::joinable!(item_tags -> tags (tag_id));
diesel::joinable!(items -> item_types (item_type));
diesel::joinable!(reviews -> cards (card_id));

diesel::allow_tables_to_appear_in_same_query!(
	card_fetched_events,
	cards,
	item_relations,
	item_tags,
	item_types,
	items,
	metadata,
	reviews,
	tags,
);
