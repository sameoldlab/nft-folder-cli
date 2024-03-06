use ethers::{
    prelude::*,
    providers::{Middleware, Provider},
    utils::hex,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{to_value, Value};
use std::{
    env,
    error::Error,
};

const RPC_URL: &str = "https://eth.llamarpc.com";

#[derive(Serialize, Deserialize, Debug)]
struct Nft {
    id: String,
    contract_address: String,
    token_id: String,
    name: String,
    image_url: String,
    // Include other fields as needed
}

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

    // let data = &response_json["id"]["tokens"]["nodes"];
    println!("{:#?}", response_json);
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

async fn request_nft_collection(address: &str) -> Result<Value, Box<dyn Error>> {
    let client = Client::new();

    let query = format!(
        r#"
		query NFTsForAddress {{
			tokens(networks: [{{network: ETHEREUM, chain: MAINNET}}],
						pagination: {{limit: 3}},
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

    let response = client
        .post("https://api.zora.co/graphql")
        .json(&request_body)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;

    Ok(response)
}
