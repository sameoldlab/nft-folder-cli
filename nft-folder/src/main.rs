use ethers::{
    prelude::*,
    providers::Provider,
};
use eyre::Result;
use futures::stream::{self, StreamExt};
use nft_folder::{self, create_directory, handle_download, resolve_ens_name, NftResponse};
use reqwest::Client;
use std::env;
use nft_folder::{
	self,
	create_directory_if_not_exists,
	handle_download,
	resolve_ens_name,
	NftResponse
};

const RPC_URL: &str = "https://eth.llamarpc.com";

#[tokio::main]
async fn main() -> Result<()> {
    //
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ENS name>", args[0]);
        std::process::exit(1);
    }

    let ens_name = &args[1];
    let provider = Provider::<Http>::try_from(RPC_URL)?;
    let address = resolve_ens_name(ens_name, &provider).await?;
    println!("Resolved address {}", address);

    let response_json = NftResponse::request(&address).await?;

    let nodes = response_json.data.tokens.nodes;
    println!("Found {} NFTs. Starting download...", nodes.len());

    // Create the directory based on the ENS name
    create_directory(&ens_dir).await?;

    let client = Client::new();
    let max_concurrent_downloads = 5;

    // Save NFT images
    let download_tasks = stream::iter(nodes.into_iter().map(|node| {
        let ens_dir = ens_dir.clone();
        let client = client.clone();
        async move { handle_download(node, &ens_dir, &client).await }
    }))
    .buffer_unordered(max_concurrent_downloads)
    .collect::<Vec<_>>();

    let results = download_tasks.await;

    for result in results {
        if let Err(err) = result {
					println!("{err}");
            // return Err(err);
        }
    }
    Ok(())
}
