use clap::Subcommand;

use crate::client::HippocampusClient;
use crate::output::{self, OutputConfig};

/// Review management commands
#[derive(Subcommand, Debug)]
pub enum ReviewCommands {
    /// Create a new review for a card
    Create {
        /// The card ID to review
        #[clap(long)]
        card_id: String,
        /// The rating (1-4)
        #[clap(long)]
        rating: i32,
    },
}

/// Executes a review command
pub async fn execute(
    client: &HippocampusClient,
    cmd: ReviewCommands,
    config: &OutputConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ReviewCommands::Create { card_id, rating } => {
            let review = client.create_review(card_id, rating).await?;
            output::print_review(&review, config);
        }
    }
    Ok(())
}
