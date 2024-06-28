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
            "{spinner:.magenta} {wide_msg} {pos:>3}/{len} [{bar:40.yellow/}] [{elapsed_precise:.bold.blue}]",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ ")
            .tick_strings(&["⣼", "⣹", "⢻", "⠿", "⡟", "⣏", "⣧", "⣶"]);
    // let debug_style = ProgressStyle::with_template("{wide_msg}").unwrap();

    let image = token.image;
    let name = if let Some(name) = token.name {
        name
    } else if let (Some(collection_name), Some(id)) = (&token.collection_name, &token.token_id) {
        format!("{} #{}", collection_name, id)
    } else {
        return Err(eyre!("Image data not found for {:#?}", token.token_id));
    }
    .replace("/", " ")
    .replace("\\", " ");

    let msg = format!("{}", &name);

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
    } else if url.starts_with("ipfs") {
        // This is probably not going to be an image, but let's take a shot and see what happens
        // println!("{} {}", name, url);
        "png".to_string()
    } else if url.starts_with("ens") {
        // println!("{} {}", name, url);
        return Err(eyre!("{name} is not an image"));
    } else {
        let ext = url.rsplit('.').next().unwrap_or_default().to_lowercase();
        if ext.len() > 5 {
            return Err(eyre!("No suitable extension found for {} {}", name, url));
        } else {
            ext
        }
    };
    // TODO: Timeout if download takes too long
    // TODO: Maybe panic automatically on unrecognized file types
    // TODO: Some SVGs seem to be having issues

    let file_path = dir.join(format!("{name}.{extension}"));

    if file_path.is_file() {
        let pb = mp.insert(
            0,
            ProgressBar::new(100).with_message(msg).with_style(pb_style),
        );
        pb.finish_with_message(format!("Already saved {name}"));
        return Ok(());
    }

    if DEBUG {
        println!("Downloading {name} to {:?}", file_path);
    }

    if url.starts_with("data:image/svg") {
        let pb = mp.insert(
            0,
            ProgressBar::new(100).with_message(msg).with_style(pb_style),
        );
        decode_and_save(
            &url.strip_prefix("data:image/svg+xml;base64,")
                .unwrap_or(&url),
            file_path,
        )?;
        pb.finish();
    } else {
        let permit = semaphore.acquire_owned().await.unwrap();
        let pb = mp.insert(
            0,
            ProgressBar::new(100).with_message(msg).with_style(pb_style),
        );

        let url = if url.starts_with("ipfs") {
            // append IPFS gateway
            let hash = url
                .split('/')
                .into_iter()
                .find(|&part| part.starts_with("Qm"));

            match hash {
                Some(hash) => format!("https://ipfs.io/ipfs/{}", hash),
                None => {
                    // Handle the case where the hash is not found
                    pb.abandon_with_message(format!("IPFS hash not found in URL for {name}"));
                    return Err(eyre::eyre!("IPFS hash not found in URL"));
                }
            }
        } else {
            url.to_owned()
        };

        let client = client.clone();

        tokio::spawn(async move {
            // pb.set_position(i);
            match download_image(&client, &url, &file_path, &pb).await {
                Ok(()) => {
                    pb.finish_with_message(format!("Completed {name}"));

                    // Ok(())
                }
                Err(error) => {
                    pb.abandon_with_message(format!(
                        "Download failed: {} to {:?}. {}",
                        name, file_path, error
                    ));
                    // return Err(eyre::eyre!("Error downloading image {}: {}", name, error));
                }
            };

            drop(permit);
        });
    }
    Ok(())
}

async fn download_image(
    client: &Client,
    image_url: &str,
    file_path: &PathBuf,
    pb: &ProgressBar,
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
