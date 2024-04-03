use ::core::time;
use clap::{Args, Parser, Subcommand};
use console::style;
use ethers::utils::hex::encode;
use ethers_providers::{Http, Middleware, Provider};
use eyre::Result;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use nft_folder::{self, create_directory, handle_download, NftResponse};
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
                    // format!("{spinner} {} {msg}", style("INFO").bright());
                    let spinner = pending(&multi_pb, "Resolving address...".to_string());
                    let address = resolve_ens_name(&arg, &provider).await?;
                    spinner.finish_with_message(format!("Resolving address: {}", address));

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
            spinner
                .finish_with_message(format!("Found {} NFTs. Starting download...", nodes.len()));

            let path = match account.name {
                Some(name) => args.path.join(name),
                None => args.path.join(account.address),
            };

            match create_directory(&path).await {
                Ok(path) => path,
                Err(err) => return Err(eyre::eyre!("{} {err}", style("Invalid Path").red())),
            };

            let client = Client::new();

            let main_pb = multi_pb.add(ProgressBar::new(nodes.len() as u64));
            main_pb.set_style(
                ProgressStyle::with_template(
                    "Total ({pos:>7}/{len:7}) {wide_bar.cyan/blue} {percent}",
                )
                .unwrap()
                .progress_chars("█░ "),
            );
            /*
               :: Remove make dependencies after install? [y/N]
               :: (1/6) ENS Name Detected. Resolving name
               :: (2/6) Name resolved to 0x21B0...42fa
               :: (3/6) Saving files to name.eth
               :: (4/6) Requesting NFT Data
               :: (5/6) 45 NFTs found. Starting download
            */
            let (tx, rx): (mpsc::Sender<DownloadResult>, mpsc::Receiver<DownloadResult>) =
                mpsc::channel(100);

            let tasks = stream::iter(nodes.into_iter().map(|node| {
                let pb = multi_pb.insert_before(&main_pb, ProgressBar::new(0));
                pb.set_style(
                    ProgressStyle::with_template(
                        "{wide_msg} {pos:>7}/{len:7} {bar.cyan/blue} {percent}",
                    )
                    .unwrap()
                    .progress_chars("█░ "),
                );
                let path = path.clone();
                let client = client.clone();
                let tx = tx.clone();
                async move {
                    // test_progress(node, tx).await;
                    handle_download(node, &path, &client).await
                }
            }))
            .buffer_unordered(args.max_concurrent_downloads);

            tasks
                .for_each(|result| async {
                    match result {
                        Ok(()) => {
                            // println!("finished with success");
                            main_pb.inc(1);
                        }
                        Err(_err) => todo!("save output for failed downloads"), // println!("finished with err"),
                    }
                })
                .await;

            main_pb.finish();
            Ok(())
        }
    }
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

    spinner.set_prefix(format!("{}", style("INFO").bold().on_blue()));
    spinner.set_message(msg);
    spinner.enable_steady_tick(time::Duration::from_millis(100));

    spinner
}
