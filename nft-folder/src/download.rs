use crate::request::{NftImage, NftNode};
use base64::decode;
use eyre::{eyre, Result};
use futures::{stream::StreamExt, Stream};
use reqwest::Client;
use std::{fs, io::Cursor, path::PathBuf, time::Duration};
use std::{
    fs::File,
    io::{self, ErrorKind, Read, Write},
};
use tokio::{sync::mpsc, time::sleep};
use tokio_util::bytes::Bytes;

pub async fn test_progress(node: NftNode, progress_tx: mpsc::Sender<DownloadResult>) {
	let file_path = match node.token.name {
		Some(name) => name,
		None => "d".to_string()
	};

	for n in 0..100 {
		sleep(Duration::from_millis(500)).await;
		let file_path = file_path.clone();
		progress_tx.send(DownloadResult {file_path, progress: n, total: 100}).await.unwrap();
	}
}

const DEBUG: bool = false;
pub async fn handle_download(node: NftNode, dir: &PathBuf, client: &Client) -> Result<()> {
    /* Pin<Box<dyn Stream<Item = Result<DownloadResult>>>> */
    let image = &node.token.image;
    let name = match &node.token.name {
        Some(name) => name,
        None => return Err(eyre!("Image data not found for {:#?}", node)),
    };

    let (url, mime) = match image {
        NftImage::Object {
            url,
            mime_type,
            size: _,
        } => (url, mime_type),
        NftImage::Url(url) => (url, &None), //meant here
        _ => return Err(eyre!("No image URL found for {name}")),
    };

    let extension = if url.starts_with("data:image/svg") {
        "svg".to_string()
    } else if let Some(mime) = mime {
        mime.rsplit("/").next().unwrap_or_default().to_string()
    } else {
        url.rsplit('.').next().unwrap_or_default().to_lowercase()
    };

    let file_path = dir.join(format!("{name}.{extension}"));
		
    if file_path.is_file() {
        if DEBUG {
            println!("Skipping {name}");
        }
        // progress.inc(1);
        return Ok(());
    }

    if DEBUG {
        println!("Downloading {name} to {:?}", file_path);
    }

    let (progress_tx, mut _progress_rx) = mpsc::channel(10); // Adjust the buffer size as needed
    match url {
        // Decode and save svg
        url if url.starts_with("data:image/svg") => save_base64_image(
            &url.strip_prefix("data:image/svg+xml;base64,")
                .unwrap_or(&url),
            file_path,
        )?,
        // append IPFS gateway
        url if url.starts_with("ipfs") => {
            let parts: Vec<&str> = url.split('/').collect();
            if let Some(hash) = parts.iter().find(|&&part| part.starts_with("Qm")) {
                let ipfs_url = format!("https://ipfs.io/ipfs/{hash}");
                if let Err(error) = download_image(&client, &ipfs_url, file_path, progress_tx).await {
                    return Err(eyre::eyre!("Error downloading image {}: {}", name, error));
                }
            }
        }
        url => {
            if let Err(error) = download_image(&client, &url, file_path, progress_tx).await {
                return Err(eyre::eyre!("Error downloading image {}: {}", name, error));
            };
        }
    }

    if DEBUG {
        println!("{name} saved successfully");
    }

    Ok(())
}
// async fn get_address()

pub struct DownloadProgress {
    pub name: String,
    pub progress: u64,
    pub total: u64,
}

#[derive(Debug)]
pub struct DownloadResult {
    file_path: String,
    progress: u64,
    total: u64,
}

struct ProgressTracker {
    progress: u64,
}
impl ProgressTracker {
    fn new() -> Self {
        ProgressTracker { progress: 0 }
    }

    // async fn track_progress<R: Read + Unpin>(
    async fn track_progress<R: Stream<Item = Result<Bytes>> + Unpin>(
        &mut self,
        index: usize,
        mut reader_stream: R,
        mut file: File,
        progress_tx: &mpsc::Sender<(usize, u64)>,
    ) -> Result<()> {
        let mut buffer = [0; 8192];
        while let Some(chunk_result) = reader_stream.next().await {
            let chunk = match chunk_result {
                Ok(chunk) => chunk,
                Err(e) => return Err(e.into()),
            };

            let mut cursor = Cursor::new(chunk);
            let bytes_read = cursor.read(&mut buffer)?;
            file.write_all(&buffer[..bytes_read])?;
            self.progress += bytes_read as u64;

            match progress_tx.try_send((index, self.progress)) {
                Ok(_) => {
                    // The progress update was sent successfully.
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // The receiver's buffer is full, you can either:
                    // 1. Drop the progress update and continue downloading
                    // 2. Wait for the receiver to process some messages before sending more updates
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // The receiver was dropped, so we stop sending progress updates.
                    break;
                }
            }
        }
        Ok(())
    }
}

async fn download_image(
    client: &Client,
    image_url: &str,
    file_path: PathBuf,
    progress_tx: mpsc::Sender<(u64, u64)>,
) -> Result<()> {
    let response = client.get(image_url).send().await?;
    let content_length = response.content_length().unwrap_or(0);
    let mut byte_stream = response.bytes_stream();

    let mut progress: u64 = 0; 
    let mut file = File::create(file_path)?;

    while let Some(chunk) = byte_stream.next().await {
			let chunk = chunk?;
			let chunk_len = chunk.len();

			progress += chunk_len as u64;
			file.write_all(&chunk)
					.map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

			// Send progress update through the channel
			let _ = progress_tx.send((progress, content_length)).await;
	}

    if content_length != progress {
        return Err(eyre::eyre!(
            "Downloaded file size does not match the expected size"
        ));
    }

    Ok(())
}

pub async fn create_directory(dir_path: &PathBuf) -> Result<PathBuf>
 {
    let res  = match fs::metadata(dir_path) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("{:?} is not a directory", dir_path),
                )
                .into());
            }
						dir_path.to_path_buf()
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            fs::create_dir_all(dir_path)?;
            if DEBUG { println!("created directory: {:?}", dir_path);}
						dir_path.to_path_buf()
        }
        Err(e) => {
            return Err(e.into());
        }
    };
    Ok(res)
}

fn save_base64_image(base64_data: &str, file_path: PathBuf) -> Result<()> {
    let decoded_data = decode(base64_data)?;
    let mut file = File::create(file_path)?;
    file.write_all(&decoded_data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
	/*
	use super::*;

    #[test]
    async fn resolve() {
        let provider: Provider<Http> = Provider::<Http>::try_from("https://eth.llamarpc.com");

        let address = &provider.resolve_name("tygra.eth").await;
        let result = format!("0x{}", encode(address));
        // let result = resolve_ens_name("tygra.eth", &provider);

        assert_eq!(result, "0x");
    }
		*/
	}
