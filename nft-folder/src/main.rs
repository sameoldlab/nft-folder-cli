use ::core::time;
use clap::{arg, Parser};
use console::style;
use ethers::utils::hex::encode;
use ethers_providers::{Http, Middleware, Provider};
use eyre::Result;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use nft_folder::{self, create_directory, handle_download, NftResponse};
use reqwest::Client;
use std::env;
const RPC_URL: &str = "https://eth.llamarpc.com";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Address as ENS Name or hex (0x1Bca23...)
    address: String,

    /// directory to create nft folder [coming soon]
    #[arg(short, long, default_value = "./test")]
    path: std::path::PathBuf,

    /// maximum number of downloads to run in parallel
    #[arg(short, long = "max", default_value_t = 5)]
    max_concurrent_downloads: usize,
}
struct Account {
    name: Option<String>,
    address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let multi_pb = MultiProgress::new();
    let provider = Provider::<Http>::try_from(RPC_URL)?;

    let account = match &args.address {
        arg if arg.split(".").last().unwrap() == "eth" => {
            // format!("{spinner} {} {msg}", style("INFO").bright());
            let spinner = pending(&multi_pb, "Resolving address...".to_string());
            let address = resolve_ens_name(&arg, &provider).await?;
            spinner.set_message(format!("Resolving address: {}", address));
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
                "{} Supported formats are 0xabc12... or name.eth",
                style("Invalid address").red()
            ))
        }
    };
    // Request
    let spinner = pending(&multi_pb, "Requesting collection data...".to_string());
    let nodes = NftResponse::request(&account.address)
        .await?
        .data
        .tokens
        .nodes;
    spinner.finish();

    let path =  match account.name {
			Some(name) => args.path.join( name),
			None => args.path.join(account.address),
		};
		
		match create_directory(&path).await { 
        Ok(path) => path,
        Err(err) => return Err(eyre::eyre!("{} {err}", style("Invalid Path").red())),
    };

    let client = Client::new();
    
		let main_pb = multi_pb.add(ProgressBar::new(nodes.len() as u64));
    main_pb.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7}")
            .unwrap()
						.progress_chars("█░ ")
    );
    let tasks = stream::iter(nodes.into_iter().map(|node| {
        let ens_dir = ens_dir.clone();
        let client = client.clone();
        async move { handle_download(node, &ens_dir, &client).await }
    }))
    .buffer_unordered(args.max_concurrent_downloads);

    tasks
        .for_each(|result| async {
            match result {
                Ok(()) => {
                    println!("finished with success");
                    main_pb.inc(1);
                }
                Err(err) => println!("finished with err"),
            }
        })
        .await;

    main_pb.finish();
    Ok(())
}

async fn resolve_ens_name(ens_name: &str, provider: &Provider<Http>) -> Result<String> {
    let address = provider.resolve_name(ens_name).await?;
    Ok(format!("0x{}", encode(address)))
}

/* function which wraps a generic action with a spinner then returns it's reult */
fn pending(multi_pb: &MultiProgress, msg: String) -> ProgressBar {
    let spinner = multi_pb.add(
        ProgressBar::new_spinner().with_style(
            ProgressStyle::default_spinner()
                .template("{spinner} {prefix} {msg}")
                .unwrap(), // .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        ),
    );

    spinner.set_prefix(format!("{}", style("INFO").green()));
    spinner.set_message(msg);
    spinner.enable_steady_tick(time::Duration::from_millis(100));

    spinner
}
