use crate::request::{NftImage, NftToken};

use base64::decode;
use eyre::{eyre, Result};
use futures::stream::StreamExt;
use reqwest::Client;
use std::sync::Arc;
use std::{fs, path::PathBuf};
use std::{
    fs::File,
    io::{self, ErrorKind, Write},
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;

const DEBUG: bool = false;

pub async fn handle_token(
    semaphore: Arc<Semaphore>,
    token: NftToken,
    client: &Client,
    mp: &MultiProgress,
    dir: &PathBuf,
) -> Result<()> {
    let pb_style = ProgressStyle::with_template(
            "{spinner:.magenta} {wide_msg} [{elapsed_precise:.bold.blue}] [{bar:40.yellow/}] {pos:>3}/{len} ({eta:>3})",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ ")
            .tick_strings(&["⣼", "⣹", "⢻", "⠿", "⡟", "⣏", "⣧", "⣶"]);

    let image = token.image;
    let name = match token.name {
        Some(name) => name,
        None => return Err(eyre!("Image data not found for {:#?}", token.token_id)),
    };
    let msg = format!("downloading {}", &name);

    let (url, mime) = match image {
        NftImage::Object {
            url,
            mime_type,
            size: _,
        } => (url, mime_type),
        NftImage::Url(url) => (url, None),
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
        let pb = mp.add(ProgressBar::new(100).with_message(msg).with_style(pb_style));
        pb.finish_with_message(format!("Already downloaded {name}"));
        return Ok(());
    }

    if DEBUG {
        println!("Downloading {name} to {:?}", file_path);
    }

    if url.starts_with("data:image/svg") {
        let pb = mp.add(ProgressBar::new(100).with_message(msg).with_style(pb_style));
        decode_and_save(
            &url.strip_prefix("data:image/svg+xml;base64,")
                .unwrap_or(&url),
            file_path,
        )?;
        pb.finish();
    } else {
        let permit = semaphore.acquire_owned().await.unwrap();
        let pb = mp.add(ProgressBar::new(100).with_message(msg).with_style(pb_style));

        let url = if url.starts_with("ipfs") {
            // append IPFS gateway
            let parts: Vec<&str> = url.split('/').collect();
            let hash = parts.iter().find(|&&part| part.starts_with("Qm"));

            // Handle the case where the hash is not found
            match hash {
                Some(hash) => format!("https://ipfs.io/ipfs/{}", hash),
                None => {
                    // pb.finish_with_message(format!("IPFS hash not found in URL for {name}"));
                    return Err(eyre::eyre!("IPFS hash not found in URL"));
                } //if a single image fails I want to finish it immediately without disrupting other ongoing processess
            }
        } else {
            url.to_owned()
        };

        let client = client.clone();
        let name_cp = name.clone();

        tokio::spawn(async move {
            // pb.set_position(i);
            match download_image(&client, &url, file_path, &pb).await {
                Ok(()) => pb.finish(),
                Err(error) => {
                    pb.finish_with_message(format!(
                        "Error downloading image {}: {}",
                        name_cp, error
                    ));
                    // return Err(eyre::eyre!("Error downloading image {}: {}", name, error));
                }
            };

            drop(permit);
        });
    }

    if DEBUG {
        println!("{name} saved successfully");
    }
    Ok(())
}

async fn download_image(
    client: &Client,
    image_url: &str,
    file_path: PathBuf,
    pb: &ProgressBar
) -> Result<()> {
    let response = client.get(image_url).send().await?;
    let content_length = response.content_length().unwrap_or(100);
    let mut byte_stream = response.bytes_stream();
    pb.set_length(content_length);

    let mut progress: u64 = 0;
    let mut file = File::create(file_path)?;

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk?;
        let chunk_len = chunk.len();

        progress += chunk_len as u64;
        file.write_all(&chunk)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        pb.set_position(progress);
    }

    if content_length != progress {
        return Err(eyre::eyre!(
            "Downloaded file size does not match the expected size"
        ));
    }

    Ok(())
}

pub async fn create_directory(dir_path: &PathBuf) -> Result<PathBuf> {
    let res = match fs::metadata(dir_path) {
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
            if DEBUG {
                println!("created directory: {:?}", dir_path);
            }
            dir_path.to_path_buf()
        }
        Err(e) => {
            return Err(e.into());
        }
    };
    Ok(res)
}

fn decode_and_save(base64_data: &str, file_path: PathBuf) -> Result<()> {
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
