// mod download;
mod request;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use request::handle_processing;
use reqwest::Client;

fn download_image(url: &String, mp: &MultiProgress) {
	println!("spawning thread");

    let pb = mp.insert_from_back(0, ProgressBar::new(100));
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.magenta} [{elapsed_precise:.bold.blue}] [{bar:40.yellow/}] {pos}/{len} ({eta})",
        )
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  ")
        .tick_strings(&["⣼", "⣹", "⢻", "⠿", "⡟", "⣏", "⣧", "⣶"])
    );
    // println!("Get image {} on t:{t}", url);
    // thread::sleep(Duration::from_millis(r));
    // for i in 0..100 {
    //     let r: u64 = random::<u64>() / 600093603030000000;
    //     thread::sleep(Duration::from_millis(r));
    //     // pb.set_position(i);
    // }
    println!("url: {url}");
    // println!("Downloaded {} on t:{t}", url);
    pb.finish();
}

#[tokio::main]
async fn main() {
    let client = Client::new();
    let address = "0x495f947276749Ce646f68AC8c248420045cb7b5e";

    if let Err(e) = handle_processing(&client, address).await {
        println!("Error: {}", e);
    }
}
