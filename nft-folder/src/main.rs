mod download;
mod request;

use std::path::PathBuf;
use download::create_directory;
use request::handle_processing;

use crate::request::NftResponse;
use ::core::time;
use clap::{Args, Parser, Subcommand};
use console::style;
use ethers::utils::hex::encode;
use ethers_providers::{Http, Middleware, Provider};
use eyre::Result;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use std::borrow::Borrow;
use tokio::sync::mpsc;

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
            let nodes = NftResponse::request(&account.address).await?.tokens.nodes;
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
            let address = "0xa3a4548b39da96eb065ff91811ca30da40431c0d";
            let mut path = PathBuf::from("test");
            path.push(&address);
            // println!("{:#?}", &path);
            
            match create_directory(&path).await {
                Ok(path) => {
                    if let Err(e) = handle_processing(&client, address, path).await {
                        println!("Error: {}", e);
                    };
                    return Ok(())
                }
                Err(err) => return Err(err)
            }

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

            // let (tx, rx) = mpsc::channel::<Ret>(100);

            let tasks = total_pb
                .wrap_stream(stream::iter(nodes.into_iter().map(|node| {
                    // let tx = tx.clone();
                    async move {
                        track_task(node, &multi_pb).await
                        //  worker2(node, tx).await }
                    }
                })))
                .buffer_unordered(3)
                .collect::<Vec<_>>();

            let _x = tasks.await;
            /*             tasks
            .for_each(|result| async move {
                let pb = multi_pb.insert_from_back(1, ProgressBar::new(100));
                pb.set_style(
                    ProgressStyle::with_template(
                        "{wide_msg:!} {bytes_per_sec} {elapsed:>} {bar:40} {percent:>3}% ",
                    )
                    .unwrap()
                    .progress_chars("██ "),
                );
                match result {
                    Ok(res) => {
                        pb.set_message(res);

                        //******** */
                        while let Some(recv) = rx.recv().await {
                            let pos = recv.progress * 100 / recv.total;
                            pb.set_position(pos);
                        }
                    }
                    Err(_err) => pb.abandon_with_message("Error during download"),
                }
            })
            .await; */
            // tokio::spawn(tasks);
            // Wait for the worker thread to finish and receive the result
            // let result = rx.recv().unwrap();

            // Print the received result

            /* for recv in rx {
                pb.set_message(recv.node);
                let pos = recv.progress * 100 / recv.total;

                pb.set_position(pos);
                // println!("{:?} / {:?} = {:?}", recv.progress, recv.total, pos);
            } */
            Ok(())
        }
    }
}
async fn track_task(node: String, _multi_pb: &MultiProgress) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Ret>();

    let pb = ProgressBar::new(100);
    let _ = worker2(&node, tx);
    pb.set_message(node);
    pb.set_style(
        ProgressStyle::with_template(
            "{wide_msg:!} {bytes_per_sec} {elapsed:>} {bar:40} {percent:>3}% ",
        )
        .unwrap()
        .progress_chars("██ "),
    );
    for recv in rx {
        let pos = recv.progress * 100 / recv.total;

        pb.set_position(pos);
        // println!("{:?} / {:?} = {:?}", recv.progress, recv.total, pos);
    } /*
          while let Ok(recv) = rx.recv() {
              let pos = recv.progress * 100 / recv.total;
              println!("{pos}");
              pb.set_position(pos);
      }; */
    Ok(())
}
#[derive(Debug)]
struct Ret {
    progress: u64,
    total: u64,
}

fn worker2(node: &String, progress_tx: std::sync::mpsc::Sender<Ret>) -> Result<()> {
    let total = 100;
    for n in 0..total {
        std::thread::sleep(tokio::time::Duration::from_millis(10));
        progress_tx
            .send(Ret {
                progress: n,
                total: total,
            })
            .unwrap();
    }
    Ok(())
}

async fn resolve_ens_name(ens_name: &str, provider: &Provider<Http>) -> Result<String> {
    let address = provider.resolve_name(ens_name).await?;
    Ok(format!("0x{}", encode(address)))
}

/* function which wraps a generic action with a spinner then returns it's reult */
fn pending(multi_pb: &MultiProgress, msg: String) -> ProgressBar {
    // https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json
    let style = ProgressStyle::default_spinner()
        .template("{spinner} {prefix:.bold.blue} {msg}")
        .unwrap()
        .tick_strings(&["⣼", "⣹", "⢻", "⠿", "⡟", "⣏", "⣧", "⣶"]);
    let spinner = multi_pb.add(ProgressBar::new_spinner().with_style(style));
    spinner.set_prefix("INFO");
    spinner.set_message(msg);
    spinner.enable_steady_tick(time::Duration::from_millis(100));

    spinner
}

/*
#[tokio::main]
async fn main() -> eyre::Result<()>{
        let nodes = vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
                "fourth node".to_string(),
        ];

        let (tx, rx) = std::sync::mpsc::channel::<Ret>();
        let tasks = stream::iter(nodes.into_iter().map(|node| {
                let tx = tx.clone();
                async move { worker2(node, tx).await }
        }))
        .buffer_unordered(2);

        // indicatif Multiprogress
        let multi_pb = MultiProgress::new();
        // Tracks total progress of nodes
        let total_pb = multi_pb.add(ProgressBar::new(nodes.len().try_into()?));

        tasks
                .for_each(|result| async {
                    let pb = multi_pb.insert_before(&total_pb, ProgressBar::new(100));
                    pb.set_style(
                            ProgressStyle::with_template(
                                    "{wide_msg:!} {bytes_per_sec} {elapsed:>} {bar:40} {percent:>3}% ",
                            )
                            .unwrap()
                            .progress_chars("██ "),
                    );
                    match result {
                        Ok(res) => {
                                    pb.set_message(res);
                                    while let Ok(recv) = rx.recv() {
                                        let pos = recv.progress * 100 / recv.total;
                                        pb.set_position(pos);
                                }
                                        total_pb.inc(1);
                                }
                        Err(_err) => pb.abandon_with_message("Error during download"),
                        }
                })
                .await;
        Ok(())
} */
