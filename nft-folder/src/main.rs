mod request;
mod download;

use request::handle_processing;
use reqwest::Client;

#[tokio::main]
async fn main() {
    let client = Client::new();
    let address = "0x495f947276749Ce646f68AC8c248420045cb7b5e";

    if let Err(e) = handle_processing(&client, address).await {
        println!("Error: {}", e);
    }
}
