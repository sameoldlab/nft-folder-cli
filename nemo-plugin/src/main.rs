// use nft_folder::*;

fn main() {
    println!("Hello, world!");
}
/* 
impl FileManagerActionImpl for MyNemoPlugin {
    fn activate_action(&self, window: &gio::FileManagerWindow, files: &[gio::File]) {
        // Get the selected ENS name from the user
        let ens_name = get_ens_name_from_user();

        // Create a Tokio runtime to run the async code
        let runtime = Runtime::new().unwrap();
        let result = runtime.block_on(async {
            let provider = Provider::<Http>::try_from(RPC_URL)?;
            let address = resolve_ens_name(&ens_name, &provider).await?;

            let response_json = request_nft_collection(&address).await?;

            let client = Client::new();
            let nodes = &response_json.data.tokens.nodes;
            println!("Found {} NFTs. Starting download...", nodes.len());

            // Create the directory based on the ENS name
            let ens_dir = format!("./test/{}", ens_name);
            create_directory(&ens_dir).await?;

            // Save NFT images
            for node in nodes {
                // ...
            }

            Ok(())
        });

        // Handle any errors that occurred
        if let Err(e) = result {
            eprintln!("Error: {}", e);
        }
    }

    // Other methods here
}
 */