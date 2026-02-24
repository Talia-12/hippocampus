use chrono::{DateTime, Utc};
use hippocampus::dto::{
    CreateItemDto, CreateItemTypeDto, CreateReviewDto, CreateTagDto, GetQueryDto,
    SortPositionAction, SuspendedFilter, UpdateItemDto,
};
use hippocampus::models::{Card, Item, ItemType, Review, Tag};
use reqwest::Client;

/// HTTP client wrapper for communicating with the Hippocampus server
pub struct HippocampusClient {
    /// The base URL of the server (e.g. "http://localhost:3000")
    base_url: String,
    /// The underlying HTTP client
    client: Client,
}

/// Builds query parameters from a GetQueryDto
fn build_query_params(query: &GetQueryDto) -> Vec<(&'static str, String)> {
    let mut params: Vec<(&'static str, String)> = Vec::new();

    if let Some(ref id) = query.item_type_id {
        params.push(("item_type_id", id.clone()));
    }
    for tag_id in &query.tag_ids {
        params.push(("tag_ids", tag_id.clone()));
    }
    if let Some(ref dt) = query.next_review_before {
        params.push(("next_review_before", dt.to_rfc3339()));
    }
    if let Some(ref dt) = query.last_review_after {
        params.push(("last_review_after", dt.to_rfc3339()));
    }
    match query.suspended_filter {
        SuspendedFilter::Include => params.push(("suspended_filter", "Include".to_string())),
        SuspendedFilter::Exclude => params.push(("suspended_filter", "Exclude".to_string())),
        SuspendedFilter::Only => params.push(("suspended_filter", "Only".to_string())),
    }
    if let Some(ref dt) = query.suspended_after {
        params.push(("suspended_after", dt.to_rfc3339()));
    }
    if let Some(ref dt) = query.suspended_before {
        params.push(("suspended_before", dt.to_rfc3339()));
    }
    if let Some(sp) = query.split_priority {
        params.push(("split_priority", sp.to_string()));
    }

    params
}

impl HippocampusClient {
    /// Creates a new HippocampusClient
    ///
    /// ### Arguments
    ///
    /// * `base_url` - The base URL of the Hippocampus server
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    // ── Item Type endpoints ──────────────────────────────────────────

    /// Lists all item types from the server
    pub async fn list_item_types(&self) -> Result<Vec<ItemType>, reqwest::Error> {
        let url = format!("{}/item_types", self.base_url);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    /// Creates a new item type
    pub async fn create_item_type(&self, name: String) -> Result<ItemType, reqwest::Error> {
        let url = format!("{}/item_types", self.base_url);
        let dto = CreateItemTypeDto { name };
        let response = self.client.post(&url).json(&dto).send().await?.error_for_status()?;
        response.json().await
    }

    /// Gets a specific item type by ID
    pub async fn get_item_type(&self, id: &str) -> Result<ItemType, reqwest::Error> {
        let url = format!("{}/item_types/{}", self.base_url, id);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    // ── Item endpoints ───────────────────────────────────────────────

    /// Lists items with optional filters
    pub async fn list_items(&self, query: &GetQueryDto) -> Result<Vec<Item>, reqwest::Error> {
        let url = format!("{}/items", self.base_url);
        let params = build_query_params(query);

        let response = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?
            .error_for_status()?;
        response.json().await
    }

    /// Creates a new item
    pub async fn create_item(
        &self,
        item_type_id: String,
        title: String,
        item_data: serde_json::Value,
        priority: f32,
    ) -> Result<Item, reqwest::Error> {
        let url = format!("{}/items", self.base_url);
        let dto = CreateItemDto {
            item_type_id,
            title,
            item_data,
            priority,
        };
        let response = self.client.post(&url).json(&dto).send().await?.error_for_status()?;
        response.json().await
    }

    /// Gets a specific item by ID
    pub async fn get_item(&self, id: &str) -> Result<Option<Item>, reqwest::Error> {
        let url = format!("{}/items/{}", self.base_url, id);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    /// Updates an item
    pub async fn update_item(
        &self,
        id: &str,
        title: Option<String>,
        item_data: Option<serde_json::Value>,
    ) -> Result<Item, reqwest::Error> {
        let url = format!("{}/items/{}", self.base_url, id);
        let dto = UpdateItemDto { title, item_data };
        let response = self.client.patch(&url).json(&dto).send().await?.error_for_status()?;
        response.json().await
    }

    /// Deletes an item
    pub async fn delete_item(&self, id: &str) -> Result<(), reqwest::Error> {
        let url = format!("{}/items/{}", self.base_url, id);
        self.client.delete(&url).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Card endpoints ───────────────────────────────────────────────

    /// Lists cards with optional filters
    pub async fn list_cards(&self, query: &GetQueryDto) -> Result<Vec<Card>, reqwest::Error> {
        let url = format!("{}/cards", self.base_url);
        let params = build_query_params(query);

        let response = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?
            .error_for_status()?;
        response.json().await
    }

    /// Gets a specific card by ID
    pub async fn get_card(&self, id: &str) -> Result<Option<Card>, reqwest::Error> {
        let url = format!("{}/cards/{}", self.base_url, id);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    /// Updates a card's priority
    pub async fn update_card_priority(
        &self,
        id: &str,
        priority: f32,
    ) -> Result<Card, reqwest::Error> {
        let url = format!("{}/cards/{}/priority", self.base_url, id);
        let response = self
            .client
            .patch(&url)
            .json(&priority)
            .send()
            .await?
            .error_for_status()?;
        response.json().await
    }

    /// Suspends or unsuspends a card
    pub async fn suspend_card(&self, id: &str, suspend: bool) -> Result<(), reqwest::Error> {
        let url = format!("{}/cards/{}/suspend", self.base_url, id);
        self.client
            .patch(&url)
            .json(&suspend)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Gets all possible next review timestamps for a card
    pub async fn get_next_reviews(
        &self,
        card_id: &str,
    ) -> Result<Vec<(DateTime<Utc>, serde_json::Value)>, reqwest::Error> {
        let url = format!("{}/cards/{}/next_reviews", self.base_url, card_id);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    /// List the reviews of a card
    pub async fn list_reviews_for_card(
        &self,
        card_id: &str,
    ) -> Result<Vec<Review>, reqwest::Error> {
        let url = format!("{}/cards/{}/reviews", self.base_url, card_id);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    // ── Sort position endpoints ────────────────────────────────────────

    /// Sets a card's sort position
    pub async fn set_sort_position(
        &self,
        card_id: &str,
        action: &SortPositionAction,
    ) -> Result<serde_json::Value, reqwest::Error> {
        let url = format!("{}/cards/{}/sort_position", self.base_url, card_id);
        let response = self
            .client
            .patch(&url)
            .json(action)
            .send()
            .await?
            .error_for_status()?;
        response.json().await
    }

    /// Clears a single card's sort position
    pub async fn clear_card_sort_position(&self, card_id: &str) -> Result<(), reqwest::Error> {
        let url = format!("{}/cards/{}/sort_position", self.base_url, card_id);
        self.client.delete(&url).send().await?.error_for_status()?;
        Ok(())
    }

    /// Clears sort positions for cards matching the query
    pub async fn clear_sort_positions(&self, query: &GetQueryDto) -> Result<(), reqwest::Error> {
        let url = format!("{}/cards/sort_positions", self.base_url);
        let params = build_query_params(query);

        self.client
            .delete(&url)
            .query(&params)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ── Review endpoints ─────────────────────────────────────────────

    /// Creates a new review
    pub async fn create_review(
        &self,
        card_id: &str,
        rating: i32,
    ) -> Result<Review, reqwest::Error> {
        let url = format!("{}/reviews", self.base_url);
        let dto = CreateReviewDto { card_id: card_id.to_string(), rating };
        let response = self.client.post(&url).json(&dto).send().await?.error_for_status()?;
        response.json().await
    }

    // ── Tag endpoints ────────────────────────────────────────────────

    /// Lists all tags
    pub async fn list_tags(&self) -> Result<Vec<Tag>, reqwest::Error> {
        let url = format!("{}/tags", self.base_url);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    /// Creates a new tag
    pub async fn create_tag(&self, name: String, visible: bool) -> Result<Tag, reqwest::Error> {
        let url = format!("{}/tags", self.base_url);
        let dto = CreateTagDto { name, visible };
        let response = self.client.post(&url).json(&dto).send().await?.error_for_status()?;
        response.json().await
    }

    /// Lists tags for a specific item
    pub async fn list_tags_for_item(&self, item_id: &str) -> Result<Vec<Tag>, reqwest::Error> {
        let url = format!("{}/items/{}/tags", self.base_url, item_id);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }

    /// Adds a tag to an item
    pub async fn add_tag_to_item(
        &self,
        item_id: &str,
        tag_id: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/items/{}/tags/{}", self.base_url, item_id, tag_id);
        self.client.post(&url).send().await?.error_for_status()?;
        Ok(())
    }

    /// Removes a tag from an item
    pub async fn remove_tag_from_item(
        &self,
        item_id: &str,
        tag_id: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/items/{}/tags/{}", self.base_url, item_id, tag_id);
        self.client.delete(&url).send().await?.error_for_status()?;
        Ok(())
    }
}
