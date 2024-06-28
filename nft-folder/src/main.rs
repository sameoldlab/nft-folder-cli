mod request;
mod download;

use std::path::PathBuf;

use download::create_directory;
use eyre::Result;
use request::handle_processing;
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<()>{
    let client = Client::new();
    let address = "0xa3a4548b39da96eb065ff91811ca30da40431c0d";
    let mut path = PathBuf::from("test");
    path.push(&address);
    println!("{:#?}", &path);
    
    match create_directory(&path).await {
        Ok(path) => {
            if let Err(e) = handle_processing(&client, address, path).await {
                println!("Error: {}", e);
            };
        }
        Err(err) => return Err(err)
    }

    Ok(())
}
