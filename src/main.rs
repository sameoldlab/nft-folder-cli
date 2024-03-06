use ethers::prelude::*;
use ethers::providers::{Middleware, Provider};
use ethers::utils::hex;
use std::error::Error;
use std::env;

const RPC_URL: &str = "https://eth.llamarpc.com";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ENS name>", args[0]);
        std::process::exit(1);
    }

    let ens_name = &args[1];

    let address = resolve_ens_name(ens_name).await?;

    // let block_number: U64 = provider.get_block_number().await?;
    println!("{address}");

    Ok(())
}

async fn resolve_ens_name(ens_name: &str) -> Result<String, Box<dyn Error>> {
    let provider = Provider::<Http>::try_from(RPC_URL)?;
    let address = provider.resolve_name(ens_name).await?;
    Ok(format!("0x{}", hex::encode(address)))
}
