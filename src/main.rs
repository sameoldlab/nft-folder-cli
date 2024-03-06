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
    // io::{Read, Write},
    // net::TcpStream,
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

    // let response_json = request_nft_collection(&address)?;
    let response_json = request_nft_collection2(&address).await?;

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
/*
fn request_nft_collection(address: &str) -> Result<Value, Box<dyn Error>> {
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
    let request_body = format!(
        r#"
{{
    "query": "{}",
    "variables": null
}}
"#,
        query
    );

    let request_headers = format!(
        "POST /graphql HTTPS/1.1\r\n\
Host: api.zora.co\r\n\
Content-Type: application/json\r\n\
User-Agent: MyApp/1.0\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\r\n",
        request_body.len()
    );

    let mut stream = TcpStream::connect(("api.zora.co", 443))?;
    stream.write_all(request_headers.as_bytes())?;
    stream.write_all(request_body.as_bytes())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    println!(
        "Response headers:\n{}",
        response
            .lines()
            .take_while(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    );
    println!("Response body:\n{}", response.trim());

    let response_json: Value = serde_json::from_str(&response)?;

    Ok(response_json)
}
 */
async fn request_nft_collection2(address: &str) -> Result<Value, Box<dyn Error>> {
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
