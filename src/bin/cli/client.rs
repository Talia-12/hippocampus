use hippocampus::models::ItemType;
use reqwest::Client;

/// HTTP client wrapper for communicating with the Hippocampus server
pub struct HippocampusClient {
    /// The base URL of the server (e.g. "http://localhost:3000")
    base_url: String,
    /// The underlying HTTP client
    client: Client,
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

    /// Lists all item types from the server
    ///
    /// ### Returns
    ///
    /// A list of item types, or an error if the request fails
    pub async fn list_item_types(&self) -> Result<Vec<ItemType>, reqwest::Error> {
        let url = format!("{}/item_types", self.base_url);
        let response = self.client.get(&url).send().await?.error_for_status()?;
        response.json().await
    }
}
