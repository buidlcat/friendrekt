// Heavily based on:
// https://github.com/evmcheb/friendrekt

mod bindings;
mod bset;
mod fasthttp;
mod math;
mod prod_kosetto;

use bindings::shares::shares::shares;
use bindings::sniper::sniper::sniper;
use bset::FIFOCache;
use dotenv::dotenv;
use ethers::{prelude::*, types::transaction::eip2930::AccessList, utils::hex};
use prod_kosetto::{TwitterInfo, User};
use std::{collections::HashMap, env, str::FromStr, sync::Arc, time::Duration};
use tokio::sync::Mutex;

const MAX: u64 = 100;

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

                    // are we late? the supply_limit is the max shares
                    // we'd be willing to buy at each tier of followers
                    // this limit is checked in Sniper.sol
                    let supply_limit = match followers {
                        f if f > 1_000_000 => MAX,
                        f if f > 500_000 => 60,
                        f if f > 250_000 => 60,
                        f if f > 100_000 => 40,
                        f if f > 20_000 => 30,
                        _ => 0,
                    };

                    return Some(TwitterInfo {
                        twitter_username: response.twitterUsername,
                        twitter_user_id: response.twitterUserId,
                        followers,
                        supply_limit,
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
    let sniper_address: String =
        env::var("SNIPER_ADDRESS").expect("SNIPER_ADDRESS must be set in .env");

    let provider = Provider::<Ws>::connect(ws_url).await?;
    let cid = provider.get_chainid().await?.as_u64();
    let signer: LocalWallet = private_key
        .parse::<LocalWallet>()
        .unwrap()
        .with_chain_id(cid);

    let provider = Arc::new(SignerMiddleware::new(provider, signer));
    println!("Connected to {:?} with ChainID {}", provider.address(), cid);

    let _share_sniper = Arc::new(sniper::new(
        Address::from_str(&sniper_address).unwrap(),
        provider.clone(),
    ));

    let _friendtech = Arc::new(shares::new(
        Address::from_str(&ft_address).unwrap(),
        provider.clone(),
    ));

    let amount = U256::from(5);
    println!("-------------------");
    println!("friend.tech share calculations.\nAmount is hardcoded to 5:");
    for supply in 1..41 {
        if supply % 5 != 0 && supply != 1 {
            continue;
        }

        let price = math::get_price(U256::from(supply), amount);
        let price = math::wei_to_eth(price);
        println!("Cost for {} shares @ {}: {}", amount, supply, price);
    }

    let blockclient = provider.clone();
    tokio::spawn(async move {
        let mut stream = blockclient.subscribe_blocks().await.unwrap();
        while let Some(block) = stream.next().await {
            let block = blockclient
                .get_block_with_txs(block.hash.unwrap())
                .await
                .unwrap();

            if let Some(block) = block {
                for tx in block.transactions {
                    // Used to check for RelayMessages
                    let blockclient = blockclient.clone();
                    let address_to_info =
                        Arc::new(Mutex::new(HashMap::<Address, TwitterInfo>::new()));
                    let friendtech = _friendtech.clone();
                    let share_sniper = _share_sniper.clone();
                    let fasthttp =
                        fasthttp::FastHttp::new("https://mainnet-sequencer.base.org/".to_string());

                    tokio::spawn(async move {
                        let mut current_nonce = blockclient
                            .get_transaction_count(blockclient.address(), None)
                            .await
                            .unwrap();

                        let mut seen = FIFOCache::<H256>::new(10);
                        let buy_sig = Bytes::from_str("0x6945b123").unwrap();
                        let relay_txn_sig = Bytes::from_str("0xd764ad0b").unwrap();

                        if seen.contains(&tx.hash) {
                            return;
                        } else {
                            println!("-------------------");
                            println!("New pending tx: {:?}", tx.hash);
                            seen.insert(tx.hash);
                        }

                        if tx.input.starts_with(&buy_sig) && tx.input.len() == 68 {
                            // tx that bought shares
                            if tx.to.is_none() {
                                return;
                            }

                            if let Some(tt) = tx.transaction_type {
                                if tt != U64::from(2) {
                                    return;
                                }
                            }

                            if tx.value != U256::zero() && tx.to.unwrap() != friendtech.address() {
                                return;
                            }

                            let mut address_to_info = address_to_info.lock().await;
                            let info = match address_to_info.get(&tx.from) {
                                Some(info) => {
                                    // found twitter info cache
                                    Some(info.clone())
                                }
                                None => {
                                    if let Some(live_info) = twitter_id_search(tx.from).await {
                                        println!(
                                            "[buyShares] Put Twitter user in cache! {} – Followers: {}",
                                            live_info.twitter_user_id,
                                            live_info.followers
                                        );

                                        address_to_info.insert(tx.from, live_info);
                                        address_to_info.get(&tx.from).cloned()
                                    } else {
                                        println!(
                                            "No registered friend.tech account for {}",
                                            tx.from
                                        );
                                        None
                                    }
                                }
                            };

                            drop(address_to_info);

                            if info.is_none() {
                                return;
                            }

                            let info = info.unwrap();

                            if info.supply_limit == 0 {
                                return;
                            }

                            let share_subject = Address::from_slice(&tx.input[16..36]);
                            let max_fee = tx.max_fee_per_gas.unwrap();
                            let prio_fee = tx.max_priority_fee_per_gas.unwrap();

                            println!("-------------------");
                            println!("buyShares on a worthy subject: {:?}", share_subject);
                            println!("-------------------");
                            println!("Followers: {}", info.followers);
                            println!("Supply Limit: {}", info.supply_limit);
                            println!("\n***\n");

                            let binding = share_sniper
                                .do_snipe_many_shares(
                                    vec![tx.from],
                                    vec![U256::from(amount)],
                                    vec![U256::from(info.supply_limit)],
                                )
                                .calldata()
                                .unwrap();

                            let txn = Eip1559TransactionRequest {
                                to: Some(NameOrAddress::Address(share_sniper.address())),
                                from: Some(blockclient.address()),
                                nonce: Some(current_nonce),
                                gas: Some(U256::from(1_000_000)),
                                value: None,
                                data: Some(binding),
                                chain_id: Some(U64::from(cid)),
                                max_priority_fee_per_gas: Some(prio_fee),
                                max_fee_per_gas: Some(max_fee),
                                access_list: AccessList::default(),
                            }
                            .into();

                            let sig = blockclient
                                .sign_transaction(&txn, *txn.from().unwrap())
                                .await
                                .unwrap();

                            let raw = txn.rlp_signed(&sig);
                            let hash = fasthttp
                                .send_request(format!("0x{}", hex::encode(raw)))
                                .await;

                            println!(
                                "{} {} Sent snipe: https://basescan.org/tx/{:#?}#eventlog",
                                info.twitter_username, info.followers, hash
                            );

                            current_nonce += U256::one();
                        } else if tx.input.starts_with(&relay_txn_sig) {
                            // From my testing, I haven't seen any relay_txn_sig txns come through.
                            // Could be a bug in my code, but I suspect it's just not used anymore.
                            let address_to_info_2 = address_to_info.clone();

                            let event = blockclient.get_transaction_receipt(tx.hash).await.unwrap();
                            if event.is_none() {
                                return;
                            }

                            let event = event.unwrap();
                            let deposit_event = event.logs.iter().find(|e| e.topics[0] == H256::from_str("0xb0444523268717a02698be47d0803aa7468c00acbed2f8bd93a0459cde61dd89").unwrap());
                            if deposit_event.is_none() {
                                return;
                            }

                            let deposit_event = deposit_event.unwrap();
                            let address = Address::from_slice(&deposit_event.data[12..32]);
                            if address_to_info_2.lock().await.contains_key(&address) {
                                return;
                            }

                            match twitter_id_search(address).await {
                                Some(info) => {
                                    // found twitter info cache
                                    Some(info.clone())
                                }
                                None => {
                                    if let Some(live_info) = twitter_id_search(address).await {
                                        println!(
                                            "[relaxTxn] Put Twitter user in cache! {} – Followers: {}",
                                            live_info.twitter_user_id,
                                            live_info.followers
                                        );

                                        address_to_info_2.lock().await.insert(address, live_info);
                                        address_to_info_2.lock().await.get(&address).cloned()
                                    } else {
                                        println!(
                                            "No registered friend.tech account for {}",
                                            address
                                        );
                                        None
                                    }
                                }
                            };
                        } else if tx.input.len() == 0 {
                            // iiuc this is a simple ETH transfer
                            let address_to_info_3 = address_to_info.clone();

                            if tx.to.is_none() {
                                return;
                            }

                            let to = tx.to.unwrap();
                            let from = tx.from;

                            let to_info = twitter_id_search(to).await;
                            let from_info = twitter_id_search(from).await;
                            if address_to_info_3.lock().await.contains_key(&to)
                                || address_to_info_3.lock().await.contains_key(&from)
                            {
                                return;
                            }

                            if let Some(to_info) = to_info {
                                println!(
                                    "Cached (to) {} with {} followers",
                                    to_info.twitter_user_id, to_info.followers
                                );

                                address_to_info_3.lock().await.insert(to, to_info);
                            }

                            if let Some(from_info) = from_info {
                                println!(
                                    "Cached (from) {} with {} followers",
                                    from_info.twitter_user_id, from_info.followers
                                );

                                address_to_info_3.lock().await.insert(from, from_info);
                            }
                        }
                    });
                }
            }
        }
    });

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(100)).await;
    }
}
