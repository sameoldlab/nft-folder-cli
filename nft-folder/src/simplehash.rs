use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Nft {
    pub nft_id: String,
    pub chain: String,
    pub contract_address: String,
    pub token_id: String,
    pub name: String,
    pub description: String,
    pub previews: Previews,
    pub image_url: Option<String>,
    pub image_properties: Option<ImageProperties>,
    pub video_url: Option<String>,
    pub video_properties: Option<String>,
    pub audio_url: Option<String>,
    pub audio_properties: Option<String>,
    pub model_url: Option<String>,
    pub model_properties: Option<String>,
    pub background_color: Option<String>,
    pub external_url: Option<String>,
    pub created_date: String,
    pub status: String,
    pub token_count: i64,
    pub owner_count: i64,
    pub owners: Vec<Owner>,
    pub contract: Contract,
    pub collection: Collection,
    pub last_sale: Option<String>,
    pub first_created: FirstCreated,
    pub rarity: Rarity,
    pub royalty: Vec<serde_json::Value>,
    pub extra_metadata: Metadata,
}

#[derive(Deserialize, Debug)]
pub struct Previews {
    pub image_small_url: String,
    pub image_medium_url: String,
    pub image_large_url: String,
    pub image_opengraph_url: String,
    pub blurhash: String,
    pub predominant_color: String,
}

#[derive(Deserialize, Debug)]
pub struct ImageProperties {
    pub width: i64,
    pub height: i64,
    pub size: i64,
    pub mime_type: String, //"image/png"
}

#[derive(Deserialize, Debug)]
pub struct Owner {
    pub owner_address: String, // "0xfa6E0aDDF68267b8b6fF2dA55Ce01a53Fad6D8e2",
    pub quantity: i32,
    pub first_acquired_date: String, //"2022-11-05T23:24:09",
    pub last_acquired_date: String,  //"2022-11-05T23:24:09"
}

#[derive(Deserialize, Debug)]
pub struct Collection {
    pub collection_id: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub banner_image_url: Option<String>,
    pub category: Option<String>,
    pub is_nsfw: Option<String>,
    pub external_url: Option<String>,
    pub twitter_username: Option<String>,
    pub discord_url: Option<String>,
    pub instagram_username: Option<String>,
    pub medium_username: Option<String>,
    pub telegram_url: Option<String>,
    pub marketplace_pages: Vec<String>,
    pub metaplex_mint: Option<String>,
    pub metaplex_first_verified_creator: Option<String>,
    pub floor_prices: Vec<serde_json::Value>,
    pub distinct_owner_count: i64,
    pub distinct_nft_count: i64,
    pub total_quantity: i64,
    pub top_contracts: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Contract {
    pub r#type: String,
    pub name: String,
    pub symbol: String,
    pub deployed_by: String,
    pub deployed_via_contract: String,
}


#[derive(Deserialize, Debug)]
pub struct FirstCreated {
    pub minted_to: String, //"0xfa6E0aDDF68267b8b6fF2dA55Ce01a53Fad6D8e2",
    pub quantity: i64,
    pub timestamp: String, //"2022-11-05T23:24:09",
    pub block_number: i64,
    pub transaction: String, //"0xd6e4bde3732edc53414cb055c23c279367c1231c31eec24080e2139be676f02d",
    pub transaction_initiator: String, //"0xd901d97D3Ab294E1E883d7EBcc39bF77Cf6b18f8"
}

#[derive(Deserialize, Debug)]
pub struct Rarity {
    pub rank: i32,
    pub score: i32,
    pub unique_attributes: i32,
}

#[derive(Deserialize, Debug)]
pub struct Metadata {
    pub attributes: Vec<serde_json::Value>,
    //   trait_type: String,//"number",
    //   value: String,
    //   display_type: Option<String>
    // -----------------------------
    pub properties: serde_json::Value,
    // Creator: String
    // number: i64,
    // name: String
    pub image_original_url: String,
    pub animation_original_url: Option<String>,
    pub metadata_original_url: String,
}
