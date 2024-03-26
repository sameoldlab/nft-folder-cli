use base64::decode;
use ethers::{
    prelude::*,
    providers::{Middleware, Provider},
    utils::hex,
};
use eyre::{eyre, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::{
    fs::File,
    io::{self, ErrorKind, Write},
};
use tokio::fs;

const DEBUG: bool = false;
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

    let file_path = format!("{ens_dir}/{name}.{extension}");

    /* Need to check if the file exist, but don't reliably know the file extension till download_image_auto */
    if file_exists(&file_path).await {
        if DEBUG {
            println!("Skipping {name}");
        }
        return Ok(());
    }

    if DEBUG {
        println!("Downloading {name} to {file_path}");
    }

    match url {
        // Decode and save svg
        url if url.starts_with("data:image/svg") => save_base64_image(
            &url.strip_prefix("data:image/svg+xml;base64,")
                .unwrap_or(&url),
            &file_path,
        )?,
        // append IPFS gateway
        url if url.starts_with("ipfs") => {
            let parts: Vec<&str> = url.split('/').collect();
            if let Some(hash) = parts.iter().find(|&&part| part.starts_with("Qm")) {
                let ipfs_url = format!("https://ipfs.io/ipfs/{hash}");
                if let Err(error) = download_image(&client, &ipfs_url, &file_path).await {
                    return Err(eyre::eyre!("Error downloading image {}: {}", name, error));
                }
            }
        }
        url => {
            if let Err(error) = download_image(&client, &url, &file_path).await {
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
pub async fn resolve_ens_name(ens_name: &str, provider: &Provider<Http>) -> Result<String> {
    let address = provider.resolve_name(ens_name).await?;
    Ok(format!("0x{}", hex::encode(address)))
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
#[serde(rename_all = "camelCase")]
pub enum NftImage {
    Null,
    Url(String),
    Object {
        url: String,
        size: Option<serde_json::Value>,
        mime_type: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]

pub struct NftToken {
    pub image: NftImage,
    pub name: Option<String>,
    pub collection_name: Option<String>,
    pub token_url: Option<String>,
    pub token_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NftNode {
    token: NftToken,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct NftTokens {
    pub nodes: Vec<NftNode>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct NftData {
	pub tokens: NftTokens,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct NftResponse {
	pub data: NftData,
}

impl NftResponse {
    pub async fn request(address: &str) -> Result<NftResponse> {
        let query = format!(
            r#"
		query NFTsForAddress {{
			tokens(networks: [{{network: ETHEREUM, chain: MAINNET}}],
						pagination: {{limit: 32}},
						where: {{ownerAddresses: "{}"}}) {{
				nodes {{
					token {{
						tokenId
						tokenUrl
						collectionName
						name
						image {{
							url
							size
							mimeType
						}}
						metadata
					}}
				}}
			}}
		}}
		"#,
            address
        );

        let request_body = to_value(serde_json::json!({
                        "query": query,
                        "variables": null,
        }))?;

        let response = Client::new()
            .post("https://api.zora.co/graphql")
            .json(&request_body)
            .send()
            .await
            .map_err(|err| eyre!("Failed to send request: {}", err))?;
        let mut response_body = response.bytes_stream();

        let mut response_data = Vec::new();
        while let Some(item) = response_body.next().await {
            let chunk = item.map_err(|err| eyre!("Failed to read response: {}", err))?;
            response_data.extend_from_slice(&chunk);
        }

        let response_str = String::from_utf8(response_data)
            .map_err(|err| eyre!("Failed to convert response to string: {}", err))?;
        if DEBUG {
            println!("{}", &response_str);
        }
        let response: NftResponse = serde_json::from_str(&response_str)
            .map_err(|err| eyre!("Failed to parse JSON response: {}", err))?;
        if DEBUG {
            println!("{:#?}", &response.data.tokens.nodes);
        }

        Ok(response)
    }
}

async fn download_image(client: &Client, image_url: &str, file_path: &str) -> Result<()> {
    let response = client.get(image_url).send().await?;
    let bytes = response /* .error_for_status()? */
        .bytes()
        .await?;
    let mut file = File::create(file_path)?;

    file.write_all(&bytes)?;
    // copy(&mut cursor, &mut file)?;
    Ok(())
}

async fn _download_image_auto(client: &Client, image_url: &str, file_dir: &str) -> Result<String> {
    // Send a GET request to the URL
    let response = client.get(image_url).send().await?;

    // Check for HTTP errors
    let response_status = response.status();
    if !response_status.is_success() {
        let error_message = format!(
            "Error fetching {}: received HTTP status {}",
            image_url, response_status
        );
        return Err(eyre!(error_message));
    }

    // Extract the file name and type from the response headers
    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|value| value.to_str().ok().map(|s| s.to_string()));

    let bytes = response /* .error_for_status()? */
        .bytes()
        .await?;
    let file_path = format!(
        "{file_dir}.{}",
        &content_type
            .unwrap_or_default()
            .rsplit('/')
            .next()
            .unwrap_or("")
    );
    let mut file = File::create(&file_path)?;

    file.write_all(&bytes)?;
    // copy(&mut cursor, &mut file)?;
    Ok(file_path)
}

pub async fn create_directory(dir_path: &str) -> Result<()> {
    match fs::metadata(dir_path).await {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("{dir_path} is not a directory"),
                )
                .into());
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            fs::create_dir_all(dir_path).await?;
            println!("created directory: {dir_path}");
        }
        Err(e) => {
            return Err(e.into());
        }
    }
    Ok(())
}

async fn file_exists(file_path: &str) -> bool {
    fs::metadata(file_path)
        .await
        .map_or(false, |metadata| metadata.is_file())
}

fn save_base64_image(base64_data: &str, file_path: &str) -> Result<()> {
    let decoded_data = decode(base64_data)?;
    let mut file = File::create(file_path)?;
    file.write_all(&decoded_data)?;
    Ok(())
}
