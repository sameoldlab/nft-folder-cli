use eyre::{eyre, Result};
use futures::{stream, StreamExt};
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
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
    limit: i32,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NftNodes {
    pub nodes: Vec<NftNode>,
    pub page_info: PageInfo,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct NftData {
    pub tokens: NftNodes,
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
pub struct NftResponse {
    data: Option<NftData>,
    error: Option<FailedRequest>,
}

impl NftResponse {
    fn handle_errors(self) -> Option<NftData> {
        match self.data {
            Some(data) => Some(data),
            None => {
                eprintln!("Errors: {:?}", self.error);
                None
            }
        }
    }
}

pub async fn fetch_page(
    client: &Client,
    cursor: Option<String>,
    address: &str,
) -> Result<NftNodes> {
    let cursor = match cursor {
        Some(c) => format!(r#", after: "{}""#, c),
        None => "".to_owned(),
    };

    let query = format!(
        r#"
            query NFTsForAddress {{
                tokens(networks: [{{network: ETHEREUM, chain: MAINNET}}],
                    pagination: {{limit: 2 {} }},
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
                        pageInfo {{
                            endCursor
                            hasNextPage
                            limit
                        }}
                    }}
                }}
            "#,
        cursor, address
    );

    let request_body = to_value(serde_json::json!({
                        "query": query,
                        "variables": null,
    }))?;

    let response = client
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

    let response: NftResponse = serde_json::from_str(&response_str).map_err(|err| {
        // eprintln!("{}", &response_str);
        eyre!("Failed to parse JSON response: {}", err)
    })?;

    let data = response.handle_errors().unwrap();
    Ok(data.tokens)
}

pub async fn handle_processing(client: &Client, address: &str) -> eyre::Result<()> {
    client: &'a Client,
    address: &'a str,
) -> impl Stream<Item = eyre::Result<NftToken>> + 'a {
    let cursor = None;
    let requests = stream::unfold(cursor, move |cursor| async move {
        match fetch_page(&client, cursor, address).await {
            Ok(response) => {
                let items = stream::iter(response.nodes.into_iter().map(|node| node.token));
                let next_cursor = if response.page_info.has_next_page {
                    response.page_info.end_cursor
                } else {
                    None
                };
                // Max 30 requests per min to public Zora API
                std::thread::sleep(std::time::Duration::from_millis(2000));
                Some((items, next_cursor))
            }
            Err(err) => {
                eprintln!("Error fetching data: {}", err);
                None
            }
        }
    })
    .flatten();
    tokio::pin!(requests);

    while let Some(data) = requests.next().await {
        println!("{:#?}", data);
    }

    Ok(())
}
