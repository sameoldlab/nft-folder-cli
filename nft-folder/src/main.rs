mod download;
mod request;

use download::create_directory;
use request::handle_processing;

use ::core::time::Duration;
use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};
use console::style;
use eyre::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;

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
}

#[derive(Args)]
struct CreateArgs {
    /// Address as ENS Name or hex (0x1Bca23...)
    address: String,

    /// directory to create nft folder
    #[arg(short, long)]
    path: Option<PathBuf>,

    /// maximum number of parallel downloads
    #[arg(short, long = "max", default_value_t = 5)]
    max_concurrent_downloads: usize,

    /// RPC Url
    #[arg(long, default_value = "https://eth.llamarpc.com")]
    rpc: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Create(args) => {
            let multi_pb = MultiProgress::new();

            let account = args.address;
            let mut path = args
                .path
                .map(PathBuf::from)
                .or_else(|| dirs::picture_dir())
                .unwrap_or_else(|| PathBuf::from("."));
            path.push("nft-folder");

            path = path.join(&account);

            let spinner = pending(
                &multi_pb,
                format!("Saving files to {}", path.to_string_lossy()),
            );
            path = match create_directory(path).await {
                Ok(path) => {
                    spinner.finish();
                    path
                }
                Err(err) => return Err(eyre::eyre!("{} {err}", style("Invalid Path").red())),
            };

            let client = Client::new();
            handle_processing(
                &client,
                account.as_str(),
                path,
                args.max_concurrent_downloads,
            )
            .await?;

            /*
               :: (4/6) Requesting NFT Data
               :: (5/6) 45 NFTs found. Starting download
            */
            Ok(())
        }
    }
}

/// Wrapsa generic action with a spinner then return it's result
fn pending(multi_pb: &MultiProgress, msg: String) -> ProgressBar {
    // https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json
    let style = ProgressStyle::default_spinner()
        .template("{spinner:.green} {prefix:.bold.blue} {msg}")
        .unwrap()
        .tick_strings(&["⣼", "⣹", "⢻", "⠿", "⡟", "⣏", "⣧", "⣶", "✔"]);
    let spinner = multi_pb.add(ProgressBar::new_spinner().with_style(style));
    spinner.set_prefix("INFO");
    spinner.set_message(msg);
    spinner.enable_steady_tick(Duration::from_millis(100));

    spinner
}
