mod download;
mod request;

use download::create_directory;
use request::handle_processing;

use ::core::time;
use clap::{Args, Parser, Subcommand};
use console::style;
use ethers::utils::hex::encode;
use ethers_providers::{Http, Middleware, Provider};
use eyre::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use std::borrow::Borrow;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a folder for the provided address
    Create(CreateArgs),
    Test,
}

#[derive(Args)]
struct CreateArgs {
    /// Address as ENS Name or hex (0x1Bca23...)
    address: String,

    /// directory to create nft folder
    #[arg(short, long, default_value = "./test")]
    path: std::path::PathBuf,

    /// maximum number of parallel downloads
    #[arg(short, long = "max", default_value_t = 5)]
    max_concurrent_downloads: usize,

    /// RPC Url
    #[arg(long, default_value = "https://eth.llamarpc.com")]
    rpc: String,
}

struct Account {
    name: Option<String>,
    address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Create(args) => {

            let multi_pb = MultiProgress::new();
            let provider = Provider::<Http>::try_from(args.rpc)?;
            let account = match args.address {
                arg if arg.split(".").last().unwrap() == "eth" => {
                    let spinner = pending(&multi_pb, "ENS Detected. Resolving address...".to_string());
                    let address = resolve_ens_name(&arg, provider).await?;
                    spinner.finish_with_message(format!("Name Resolved to {address}"));                    
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

            let path = match account.name {
                Some(name) => {
                    let spinner = pending(&multi_pb, format!("Saving files to {}", name));
                    match create_directory(args.path.join(name)).await {
                        Ok(path) => {
                            spinner.finish();
                            path
                        },
                        Err(err) => return Err(eyre::eyre!("{} {err}", style("Invalid Path").red())),
                    }
                }
                None => args.path.join(&account.address),
            };

            let client = Client::new();
            if let Err(e) = handle_processing(&client, account.address.as_str(), path, args.max_concurrent_downloads).await {
                println!("Error: {}", e);
            };

            /*
               :: (1/6) ENS Name Detected. Resolving name
               :: (2/6) Name resolved to 0x21B0...42fa
               :: (3/6) Saving files to name.eth
               :: (4/6) Requesting NFT Data
               :: (5/6) 45 NFTs found. Starting download
            */
            Ok(())
        }

        Commands::Test => {
            let nodes = vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
                "fourth node".to_string(),
            ];

            // indicatif Multiprogress
            // Tracks total progress of nodes
            let multi_pb = MultiProgress::new();
            let multi_pb = multi_pb.borrow();
            let total_pb = multi_pb.add(ProgressBar::new(nodes.len().try_into()?));
            total_pb.set_style(
                ProgressStyle::with_template(
                    "Total [{pos:>}/{len:>}] {elapsed:>} {bar:40} {percent:>3}% ",
                )
                .unwrap()
                .progress_chars("█░ "),
            );

            Ok(())
        }
    }
}

async fn resolve_ens_name(ens_name: &str, provider: Provider<Http>) -> Result<String> {
    let address = provider.resolve_name(ens_name).await?;
    Ok(format!("0x{}", encode(address)))
}

/// Wrapsa generic action with a spinner then return it's result
fn pending(multi_pb: &MultiProgress, msg: String) -> ProgressBar {
    // https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json
    let style = ProgressStyle::default_spinner()
        .template("{spinner:.magenta} {prefix:.bold.blue} {msg}")
        .unwrap()
        .tick_strings(&["⣼", "⣹", "⢻", "⠿", "⡟", "⣏", "⣧", "⣶", "⣿"]);
    let spinner = multi_pb.add(ProgressBar::new_spinner().with_style(style));
    spinner.set_prefix("INFO");
    spinner.set_message(msg);
    spinner.enable_steady_tick(time::Duration::from_millis(100));

    spinner
}
