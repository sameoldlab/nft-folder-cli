use eyre::{eyre, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::to_value;

const DEBUG: bool = false;
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
    pub token: NftToken,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    end_cursor: String,
    has_next_page: bool,
    limit: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NftTokens {
    pub nodes: Vec<NftNode>,
    #[serde(rename = "camelCase")]
    pub page_info: PageInfo,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct NftData {
    pub tokens: NftTokens,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct FailedRequest {
    message: String,
    locations: Vec<ErrorLocation>,
    path: Vec<String>,
}
#[derive(Deserialize, Serialize, Debug)]
struct ErrorLocation {
    line: u64,
    column: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum NftResponse {
    Success { data: NftData },
    Error { errors: FailedRequest },
}

impl NftResponse {
    fn handle_errors(self) -> Option<NftData> {
        match self {
            NftResponse::Success { data } => Some(data),
            NftResponse::Error { errors } => {
                println!("Errors: {:?}", errors);
                None
            }
        }
    }

    pub async fn request(address: &str) -> Result<NftData> {
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
					pageInfo {{
					    endCursor
					    hasNextPage
					    limit
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
        while let Some(item) = futures::StreamExt::next(&mut response_body).await {
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

        let data = response.handle_errors().unwrap();
        if DEBUG {
            println!("{:#?}", &data);
        }

        Ok(data)
    }
}
