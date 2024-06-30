use crate::download::handle_token;
use eyre::{eyre, Result};
use futures::{stream, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;

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
                    pagination: {{limit: 20 {} }},
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

    let response: NftResponse = serde_json::from_str(&response_str)
        .map_err(|err| eyre!("Failed to parse JSON response: {}", err))?;
    // println!("{:?}", response);

    let data = response.handle_errors().unwrap();
    Ok(data.tokens)
}

enum PageResult {
    Data(NftToken),
    Completed,
}
pub async fn handle_processing(client: &Client, address: &str, path: PathBuf, max: usize) -> eyre::Result<()> {
    let cursor = None;
    let requests = stream::unfold(cursor, move |cursor| async move {
        match fetch_page(&client, cursor, address).await {
            Ok(response) => {
                let items = stream::iter(response.nodes.into_iter().map(move |node| {
                    if response.page_info.has_next_page {
                        PageResult::Data(node.token)
                    } else {
                        PageResult::Completed
                    }
                }));
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

    let semaphore = Arc::new(Semaphore::new(max));
    let mp = MultiProgress::new();

    mp.set_alignment(indicatif::MultiProgressAlignment::Bottom);
    let total_pb = mp.add(ProgressBar::new(0));
    total_pb.set_style(ProgressStyle::with_template(
        "Found: {len:>3.bold.blue}  Saved: {pos:>3.bold.blue} {msg}",
        )
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏ "));
        // .tick_strings(&["⣼", "⣹", "⢻", "⠿", "⡟", "⣏", "⣧", "⣶"]));
    // total_pb.set_message("Complete");
    

    while let Some(token) = requests.next().await {
        total_pb.inc_length(1);
        // let url = token.token_url.unwrap();
        match token {
            PageResult::Data(token) => {
                // println!("Sending {:?}", token.name);
                match handle_token(Arc::clone(&semaphore), token, &client, &mp, &path).await {
                    Ok(()) => {
                        total_pb.inc(1);
                    }
                    Err(err) => {
                        total_pb.println(format!("{}", err));
                    }
                }
            }
            PageResult::Completed => {
                // total_pb.abandon_with_message("Completed Sucessfully");
                total_pb.abandon();
                return Ok(())
            },
        }
    }
    Ok(())
}
