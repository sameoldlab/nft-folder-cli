#![allow(dead_code)]
use crate::download::handle_token;
use crate::simplehash::{Nft, SHResponse};

use async_stream::try_stream;
use dotenv::dotenv;
use eyre::{Report, Result};
use futures::{Stream, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, StatusCode};
use std::env;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::{sync::Semaphore, task::JoinSet};

#[derive(Error, Debug)]
enum RequestError {
    #[error("Request failed: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("API returned error: {status} - {message}")]
    SimpleHashResponseError { status: StatusCode, message: String },
    #[error("Environment variable not found: {0}")]
    EnvError(#[from] std::env::VarError),
}


fn query_address<'a>(
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

pub async fn handle_processing(address: &str, path: PathBuf, max: usize) -> eyre::Result<()> {
    let tokens = query_address(address, None);
    tokio::pin!(tokens);

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
    while let Some(token) = tokens.next().await {
        total_pb.inc_length(1);
        match token {
            Ok(token) => match handle_token(Arc::clone(&semaphore), token, &client, &mp, &path) {
                Ok(Some(task)) => {
                    set.spawn(task);
                }
                Ok(None) => total_pb.inc(1),
                Err(err) => errors.push(err),
            },
            Err(err) => return Err(err.into()),
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
