mod download;
mod request;
mod simplehash;

use download::create_directory;
use request::query_address;
use tokio::{sync::Semaphore, task::JoinSet};

use ::core::time::Duration;
use std::{ path::PathBuf, sync::Arc};
use clap::{Args, Parser, Subcommand};
use console::style;
use eyre::{Result, Report};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use futures::StreamExt;
use reqwest::Client;
use crate::download::handle_token;

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

            handle_processing(
                &account,
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

async fn handle_processing(address: &str, path: PathBuf, max: usize) -> eyre::Result<()> {
    let tokens = query_address(address, None);

    let mp = MultiProgress::new();
    mp.set_alignment(indicatif::MultiProgressAlignment::Bottom);
    let total_pb = mp.add(ProgressBar::new(0));
    total_pb.set_style(
        ProgressStyle::with_template("Found: {len:>3.bold.blue}  Saved: {pos:>3.bold.blue} {msg}")
            .unwrap(),
    );

    let semaphore = Arc::new(Semaphore::new(max));
    let mut errors: Vec<Report> = vec![];
    let mut set = JoinSet::new();

    tokio::pin!(tokens);
    let client = Client::new();
    while let Some(token) = tokens.next().await {
        match token {
            Ok(token) => match handle_token(Arc::clone(&semaphore), token, &client, &mp, &path) {
                Ok(Some(task)) => {
                    set.spawn(task);
                    total_pb.inc_length(1);
                }
                Ok(None) => total_pb.inc(1),
                Err(err) => errors.push(err),
            },
            Err(err) => return Err(err.into()),
        }
    }

    while let Some(tasks) = set.join_next().await {
        let tasks = tasks.unwrap();
        match tasks.unwrap() {
            Ok(_) => {
                total_pb.inc(1);
            }
            Err(err) => {
                errors.push(err);
            }
        }
    }

    if errors.is_empty() {
        total_pb.finish_with_message("Completed all sucessfully");
    } else {
        total_pb.abandon();
        errors.iter().for_each(|e| println!("{}", e))
    }

    Ok(())
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
