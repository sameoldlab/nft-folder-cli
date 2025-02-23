use crate::download::handle_token;
use crate::simplehash::Nft;

use async_stream::try_stream;
use dotenv::dotenv;
use eyre::{Report, Result};
use futures::{Stream, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::env;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::{sync::Semaphore, task::JoinSet};

#[derive(Error, Debug)]
pub enum RequestError {
    #[error("Request failed: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("API returned error: {status} - {message}")]
    SimpleHashResponseError { status: StatusCode, message: String },
    #[error("Environment variable not found: {0}")]
    EnvError(#[from] std::env::VarError),
}

#[derive(Deserialize, Debug)]
struct SHResponse {
    next_cursor: Option<String>,
    next: Option<String>,
    previous: Option<String>,
    nfts: Vec<Nft>,
}
#[derive(Clone)]
struct SimpleHashClient {
    client: Client,
    api_key: String,
}

impl SimpleHashClient {
    const BASE_URL: &'static str = "https://api.simplehash.com/api/v0";
    pub fn new(api_key: Option<&str>) -> Result<Self, RequestError> {
        if let Some(api_key) = api_key {
            Ok(Self {
                client: Client::new(),
                api_key: api_key.to_string(),
            })
        } else {
            dotenv().ok();
            Ok(Self {
                client: Client::new(),
                api_key: env::var("SIMPLEHASH_APIKEY")?,
            })
        }
    }
    }
    pub async fn fetch_page(
        &self,
        cursor: Option<String>,
        address: &str,
    ) -> Result<SHResponse, RequestError> {
        let mut url = format!(
            "{}/nfts/owners_v2?chains=ethereum&wallet_addresses={}&limit=50",
            SimpleHashClient::BASE_URL,
            address
        );
        if let Some(cursor) = cursor {
            url.push_str(&format!("&cursor={}", cursor));
        };
        let response = self
            .client
            .get(url)
            .header("X-API-KEY", &self.api_key)
            .header("Accept", "application/json")
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                let api_response = response.json::<SHResponse>().await?;
                Ok(api_response)
            }
            status => {
                let message = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());

                Err(RequestError::SimpleHashResponseError { status, message })
            }
        }
    }
    fn stream<'a>(
        &'a self,
        address: &'a str,
    ) -> impl Stream<Item = Result<Nft, RequestError>> + use<'a> {
         try_stream! {
            let mut current_cursor = None;
            loop {
                let response = self.fetch_page(current_cursor, &address).await?;
                for nft in response.nfts {
                    yield nft;
                }

                match response.next_cursor {
                    Some(cursor) =>  current_cursor = Some(cursor),
                    None => break,
                }
            }
        }
    }
}

pub async fn handle_processing(address: &str, path: PathBuf, max: usize) -> eyre::Result<()> {
    let sfc = SimpleHashClient::new(None)?;
    // let requests = sfc.stream_data(address.to_string()).await;

    let requests = sfc.stream(address);
    tokio::pin!(requests);

    let mp = MultiProgress::new();
    mp.set_alignment(indicatif::MultiProgressAlignment::Bottom);
    let total_pb = mp.add(ProgressBar::new(0));
    total_pb.set_style(
        ProgressStyle::with_template("Found: {len:>3.bold.blue}  Saved: {pos:>3.bold.blue} {msg}")
            .unwrap(),
    );

    let semaphore = Arc::new(Semaphore::new(max));
    let mut errors: Vec<Report> = vec![];
    let mut set = JoinSet::new();

    let client = Client::new();
    while let Some(token) = requests.next().await {
        total_pb.inc_length(1);
        match token {
            Ok(token) => {
                match handle_token(Arc::clone(&semaphore), token, &client, &mp, &path) {
                Ok(Some(task)) => {
                    set.spawn(task);
                }
                Ok(None) => total_pb.inc(1),
                Err(err) => errors.push(err),
            }
        },
            Err(err) => return Err(err.into())
        }
    }

    while let Some(tasks) = set.join_next().await {
        let tasks = tasks.unwrap();
        match tasks.unwrap() {
            Ok(_) => {
                total_pb.inc(1);
            }
            Err(err) => {
                errors.push(err);
            }
        }
    }

    if errors.is_empty() {
        total_pb.finish_with_message("Completed all sucessfully");
    } else {
        total_pb.abandon();
        errors.iter().for_each(|e| println!("{}", e))
    }

    Ok(())
}
