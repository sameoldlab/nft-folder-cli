// mod download;
mod request;

use futures::StreamExt;
use tokio::sync::{Semaphore, SemaphorePermit};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use request::request;

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (s, r) = futures::channel::mpsc::unbounded();

    let mp = MultiProgress::new();

		let receiver_stream = r.for_each(move |url| {
			let mp = mp.clone();
			tokio::spawn(async move {
				while let Some(url) = &r.next().await {
						download_image(url, &mp);
				}
		});
		});

    let address = "0x495f947276749Ce646f68AC8c248420045cb7b5e";
    let client = reqwest::Client::new();

    let stream = request(&client, &address).await;
    tokio::pin!(stream);

    while let Some(result) = stream.next().await {
        let token = result?;
				println!("received token: {:?}", token);
        let url = token.token_url.unwrap();
        if let Err(e) = s.unbounded_send(url) {
            eprintln!("Error sending url to download task: {}", e);
        }
    }

    drop(s);

    Ok(())
}
