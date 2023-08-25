// Heavily based on:
// https://github.com/evmcheb/friendrekt

mod bindings;
mod prod_kosetto;

use bindings::shares::shares::shares;
use bindings::sniper::sniper::sniper;
use dotenv::dotenv;
use ethers::prelude::*;
use prod_kosetto::{TwitterInfo, User};
use std::{env, str::FromStr, sync::Arc, time::Duration};

async fn get_followers(id: String) -> u64 {
    let req_url = format!("http://127.0.0.1:8000/{}", id);
    let client = reqwest::Client::new();
    let resp = client.get(req_url).send().await;
    if resp.is_err() || !resp.as_ref().unwrap().status().is_success() {
        println!("Failed to get followers for {}", id);
        return 0;
    }

    let data = resp.unwrap().text().await.unwrap();
    data.parse().unwrap_or(0)
}

async fn twitter_id_search(address: Address) -> Option<TwitterInfo> {
    let req_url = format!("https://prod-api.kosetto.com/users/{:?}", address);
    let client = reqwest::Client::new();

    if let Ok(resp) = client
        .get(req_url)
        .timeout(Duration::from_secs(60))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(data) = resp.text().await {
                if let Ok(response) = serde_json::from_str::<User>(&data) {
                    let followers = get_followers(response.twitterUserId.clone()).await;

                    return Some(TwitterInfo {
                        twitter_username: response.twitterUsername,
                        twitter_user_id: response.twitterUserId,
                        followers,
                        supply_limit: 0,
                    });
                }
            }
        }
    }

    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let ws_url: String = env::var("BASE_WSS_URL").expect("BASE_WSS_URL is not set");
    let private_key: String = env::var("PRIVATE_KEY").expect("PRIVATE_KEY is not set");
    let ft_address: String = env::var("FT_ADDRESS").expect("FT_ADDRESS must be set in .env");

    let provider = Provider::<Ws>::connect(ws_url).await?;
    let cid = provider.get_chainid().await?.as_u64();
    let signer: LocalWallet = private_key
        .parse::<LocalWallet>()
        .unwrap()
        .with_chain_id(cid);

    let provider = Arc::new(SignerMiddleware::new(provider, signer));

    let _friendtech = Arc::new(shares::new(
        Address::from_str(&ft_address).unwrap(),
        provider.clone(),
    ));

    let mut stream = provider.subscribe_blocks().await?.take(1);
    while let Some(block) = stream.next().await {
        let block = provider.get_block_with_txs(block.hash.unwrap()).await?;
        if let Some(block) = block {
            for tx in block.transactions {
                let buy_sig = Bytes::from_str("0x6945b123").unwrap();
                if tx.to.is_none() {
                    continue;
                }

                if let Some(tt) = tx.transaction_type {
                    if tt != U64::from(2) {
                        continue;
                    }
                }

                if tx.value != U256::zero()
                    && tx.to.unwrap() != _friendtech.address()
                    && tx.input.len() != 68
                {
                    continue;
                }

                if tx.input.starts_with(&buy_sig) {
                    let share_subject = Address::from_slice(&tx.input[16..36]);

                    println!("-------------------");
                    println!("tx hash: {:?}", tx.hash);
                    println!("buyShares executed on: {:?}", share_subject);
                    println!("-------------------");
                    println!("Performing reverse lookup on {:?}", share_subject);
                    let info = twitter_id_search(share_subject).await;
                    if let Some(info) = info {
                        println!("Found Twitter user: {}", info.twitter_user_id);
                        println!("Fetching followers...");
                        let followers = get_followers(info.twitter_user_id.clone()).await;
                        println!("Followers: {}", followers);
                    } else {
                        println!("No Twitter user found");
                    }
                    println!("\n***\n");
                }
            }
        }
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(100)).await;
    }
}
