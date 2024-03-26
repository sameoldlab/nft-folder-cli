use ::core::time;
use ethers::{prelude::*, providers::Provider};
use eyre::Result;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use nft_folder::{self, create_directory, handle_download, resolve_ens_name, NftResponse};
use reqwest::Client;
use std::env;
const RPC_URL: &str = "https://eth.llamarpc.com";

struct Account {
    name: Option<String>,
    address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    //
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ENS name>", args[0]);
        std::process::exit(1);
    }

    let provider = Provider::<Http>::try_from(RPC_URL)?;

    let account = match &args[1] {
        arg if arg.split(".").last().unwrap() == "eth" => {
            let spinner = ProgressBar::new_spinner();
            spinner.set_message("Resolving address...");
            spinner.enable_steady_tick(time::Duration::from_millis(100));
            let address = resolve_ens_name(&arg, &provider).await?;
            spinner.set_message(format!("Resolved to {}", address));
            spinner.finish();

            Account {
                name: Some(arg.to_string()),
                address,
            }
        }
        arg if arg.starts_with("0x") => Account {
            name: None,
            address: arg.to_string(),
        },
        _ => {
            return Err(eyre::eyre!(
                "Invalid address. Supported formats are 0xabc12... or name.eth"
            ))
        }
    };

    let response_json = NftResponse::request(&account.address).await?;

    let nodes = response_json.data.tokens.nodes;
    println!("Found {} NFTs. Starting download...", nodes.len());

    // Create the directory based on the ENS name
    let ens_dir = match account.name {
        Some(name) => format!("./test/{}", name),
        None => format!("./test/{}", account.address),
    };

    create_directory(&ens_dir).await?;

    let client = Client::new();
    let max_concurrent_downloads = 5;

    let pb = ProgressBar::new(nodes.len() as u64).with_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap(),
    );

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
        match result {
            Ok(()) => pb.inc(1),
            Err(err) => println!("{err}"),
        }
    }
    Ok(())
}
