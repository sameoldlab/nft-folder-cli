#![allow(dead_code)]

use crate::simplehash::{Nft, SHResponse};
use eyre::Result;
use async_stream::try_stream;
use dotenv::dotenv;
use futures::Stream;
use reqwest::{Client, StatusCode};
use thiserror::Error;
use std::env;

#[derive(Error, Debug)]
pub enum RequestError {
    #[error("Request failed: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("API returned error: {status} - {message}")]
    SimpleHashResponseError { status: StatusCode, message: String },
    #[error("Environment variable not found: {0}")]
    EnvError(#[from] std::env::VarError),
}


pub fn query_address<'a>(
    address: &'a str,
    api_key: Option<&'a str>,
) -> impl Stream<Item = Result<Nft, RequestError>> + use<'a> {
    let client = Client::new();

    try_stream! {
        let api_key = if let Some(api_key) = api_key {
            api_key.to_string()
        } else {
            dotenv().ok();
            env::var("SIMPLEHASH_APIKEY")?
        };
        let mut url = format!(
            "https://api.simplehash.com/api/v0/nfts/owners_v2?chains=ethereum&wallet_addresses={}&limit=50",
            address
        );
        loop {
            let request = client
                .get(url)
                .header("X-API-KEY", &api_key)
                .header("Accept", "application/json")
                .send()
                .await?;

            let status = request.status();
            if !status.is_success() {
                let message = request
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());

                Err(RequestError::SimpleHashResponseError { status, message })?;
                break
            }
            let response = request.json::<SHResponse>().await?;
            for nft in response.nfts {
                yield nft;
            }

            match response.next {
                Some(next) =>  url = next,
                None => break,
            };
        }
    }
}
