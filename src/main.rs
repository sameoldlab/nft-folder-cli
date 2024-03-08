use base64::decode;
use ethers::{
    prelude::*,
    providers::{Middleware, Provider},
    utils::hex,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::{
    env,
    error::Error,
    fs::File,
    io::{self, ErrorKind, Write},
};
use tokio::fs;

const RPC_URL: &str = "https://eth.llamarpc.com";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ENS name>", args[0]);
        std::process::exit(1);
    }

    let ens_name = &args[1];
    let provider = Provider::<Http>::try_from(RPC_URL)?;
    let address = resolve_ens_name(ens_name, &provider).await?;

    let response_json = request_nft_collection(&address).await?;

    let client = Client::new();
    let nodes = &response_json.data.tokens.nodes;
    println!("Found {} NFTs. Starting download...", nodes.len());

    // Create the directory based on the ENS name
    let ens_dir = format!("./test/{}", ens_name);
    create_directory_if_not_exists(&ens_dir).await?;

    // Save NFT images
    for node in nodes {
        let image = &node.token.image;
        let name: &String = &node.token.name;

        let img_url = match image {
            serde_json::Value::String(url) => Some(url.as_str()),
            serde_json::Value::Object(obj) => obj.get("url").and_then(serde_json::Value::as_str),
            _ => None,
        };
        if let Some(url) = img_url {
            let extension = if url.starts_with("data:image/svg") {
                "svg".to_string()
            } else {
                url.rsplit('.').next().unwrap_or_default().to_lowercase()
            };
            let file_path = format!("{}/{}.{}", &ens_dir, name, &extension);

            if file_exists(&file_path).await? {
                println!("Skipping {name}");
            } else {
                println!("Downloading {name}");
                if extension == "svg" {
                    save_base64_image(
                        &url.strip_prefix("data:image/svg+xml;base64,")
                            .unwrap_or(&url),
                        &file_path,
                    )?;
                } else {
                    download_image(&client, &url, &file_path).await?;
                }

                println!("{name} saved successfully");
            }
        } else {
            println!("No image URL for {}", name);
        }
    }
    // println!("{:#?}", response_json);
    Ok(())
}
// async fn get_address()
async fn resolve_ens_name(
    ens_name: &str,
    provider: &Provider<Http>,
) -> Result<String, Box<dyn Error>> {
    let address = provider.resolve_name(ens_name).await?;
    Ok(format!("0x{}", hex::encode(address)))
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]

struct NftToken {
    image: serde_json::Value,
    name: String,
    collection_address: String,
    token_id: String,
    metadata: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct NftNode {
    token: NftToken,
}
#[derive(Serialize, Deserialize, Debug)]
struct NftTokens {
    nodes: Vec<NftNode>,
}
#[derive(Serialize, Deserialize, Debug)]
struct NftData {
    tokens: NftTokens,
}
#[derive(Serialize, Deserialize, Debug)]
struct NftResponse {
    data: NftData,
}

async fn request_nft_collection(address: &str) -> Result<NftResponse, Box<dyn Error>> {
    let query = format!(
        r#"
		query NFTsForAddress {{
			tokens(networks: [{{network: ETHEREUM, chain: MAINNET}}],
						pagination: {{limit: 8}},
						where: {{ownerAddresses: "{}"}}) {{
				nodes {{
					token {{
						collectionAddress
						tokenId
						name
						owner
						image {{
							url
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
        .await?;
    let mut response_body = response.bytes_stream();

    let mut response_data = Vec::new();
    while let Some(item) = response_body.next().await {
        let chunk = item?;
        response_data.extend_from_slice(&chunk);
    }

    let response_str = String::from_utf8(response_data)?;
    // println!("{}", &response_str);

    let response: NftResponse = serde_json::from_str(&response_str)?;
    println!("{:#?}", &response.data.tokens.nodes);

    Ok(response)
}

async fn download_image(
    client: &Client,
    image_url: &str,
    file_path: &str,
) -> Result<(), Box<dyn Error>> {
    let response = client.get(image_url).send().await?;
    let bytes = response /* .error_for_status()? */
        .bytes()
        .await?;
    let mut file = File::create(file_path)?;

    file.write_all(&bytes)?;
    // copy(&mut cursor, &mut file)?;
    Ok(())
}

async fn create_directory_if_not_exists(dir_path: &str) -> Result<(), Box<dyn Error>> {
    match fs::metadata(dir_path).await {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(Box::new(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("{dir_path} is not a directory"),
                )));
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            fs::create_dir_all(dir_path).await?;
            println!("created directory: {dir_path}");
        }
        Err(e) => {
            return Err(Box::new(e));
        }
    }
    Ok(())
}

async fn file_exists(file_path: &str) -> Result<bool, Box<dyn Error>> {
    Ok(fs::metadata(file_path)
        .await
        .map_or(false, |metadata| metadata.is_file()))
}

fn save_base64_image(base64_data: &str, file_path: &str) -> Result<(), Box<dyn Error>> {
    let decoded_data = decode(base64_data)?;
    let mut file = File::create(file_path)?;
    file.write_all(&decoded_data)?;
    Ok(())
}
