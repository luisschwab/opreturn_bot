//! opreturn_bot
//!
//! checks every output on every transaction on new blocks for OP_RETURNs
//! and prints them to stdout, maybe tweet and nostr them also

use dotenv::dotenv;
use nostr_sdk::Client as NostrClient;
use nostr_sdk::EventBuilder;
use nostr_sdk::Keys;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use thousands::Separable;
use tokio::time::Duration;
use tweety_rs::TweetyClient;

const POAST_X: bool = true;
const POAST_NOSTR: bool = false;

#[rustfmt::skip]
const NOSTR_RELAYS: &[&str] = &[
    "wss://nostr.luisschwab.net",
    "wss://relay.primal.net"
    // add your relays here
];

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let env = vec![
        "RPC_URL",
        "RPC_USER",
        "RPC_PASS",
        "X_CONSUMER_KEY",
        "X_CONSUMER_SECRET",
        "X_ACCESS_TOKEN",
        "X_ACCESS_TOKEN_SECRET",
        "NOSTR_SEC",
        "BLACKLIST",
    ];

    dotenv().ok();
    let mut kv = HashMap::new();

    // save env to kv
    for var in &env {
        match env::var(var) {
            Ok(value) => {
                //println!("{}: {}", var, value);
                kv.insert(*var, value);
            }
            Err(e) => println!("couldn't read {} from environment: {}", var, e),
        }
    }

    let mut header = HeaderMap::new();
    header.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let client = Client::builder().default_headers(header).build()?;

    let blacklist: Vec<String> = fs::read_to_string(&kv["BLACKLIST"])
        .unwrap()
        .lines()
        .map(|line| line.trim().to_string())
        .collect();

    let mut last_chaintip: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_secs(5));

        // get chaintip
        let request_chaintip = json!({
            "jsonrpc": "2.0",
            "method": "getblockchaininfo",
            "params": [],
            "id": 1337
        });

        let response_chaintip = client
            .post(&kv["RPC_URL"])
            .basic_auth(&kv["RPC_USER"], Some(&kv["RPC_PASS"]))
            .json(&request_chaintip)
            .send()
            .await?
            .json::<Value>()
            .await?;

        let chaintip = response_chaintip["result"]["blocks"].as_u64().unwrap();

        let chaintip_blockhash = response_chaintip["result"]["bestblockhash"]
            .clone()
            .as_str()
            .unwrap()
            .replace("\"", "");

        if chaintip <= last_chaintip {
            continue;
        } else {
            //println!();
            //println!("new height: {}", chaintip.separate_with_commas());
            //println!("blockhash:  {}", chaintip_blockhash);
            //println!();

            last_chaintip = chaintip;
        }

        // get all transactions from chaintip block
        let request_transactions = json!({
            "jsonrpc": "2.0",
            "method": "getblock",
            "params": [
                chaintip_blockhash,
                2     // verbosity
            ],
            "id": 1337
        });

        let response_transactions = client
            .post(&kv["RPC_URL"])
            .basic_auth(&kv["RPC_USER"], Some(&kv["RPC_PASS"]))
            .json(&request_transactions)
            .send()
            .await?
            .json::<Value>()
            .await?;

        let transactions = response_transactions["result"]["tx"]
            .as_array()
            .clone()
            .unwrap();

        // gather all OP_RETURN outputs
        let mut op_returns: Vec<String> = Vec::new();
        for transaction in transactions {
            let txid = &transaction["txid"]
                .clone()
                .as_str()
                .unwrap()
                .replace("\"", "");
            let outputs = transaction["vout"].as_array().unwrap();

            for output in outputs {
                if let Some(asm) = output["scriptPubKey"]["asm"].as_str() {
                    if asm.contains("OP_RETURN") {
                        let asm: Vec<_> = asm
                            .split_whitespace()
                            .map(|s| s.trim_matches('"'))
                            .collect();

                        if let Some(last) = asm.last() {
                            match hex::decode(last) {
                                Ok(bytes) => match std::str::from_utf8(&bytes) {
                                    Ok(utf8) => {
                                        let utf8_str = utf8.to_string().trim().to_string();

                                        

                                        // filter OP_RETURN based on blacklist
                                        if !blacklist.iter().any(|item| utf8_str.contains(item)) {
                                            println!(
                                                "[{} https://mempool.space/{}] OP_RETURN: {:?}",
                                                chaintip.separate_with_commas(),
                                                txid,
                                                utf8_str.as_bytes()
                                            );
                                            op_returns.push(utf8_str);
                                        }
                                    }
                                    Err(_) => continue,
                                },
                                Err(_) => continue,
                            }
                        }
                    }
                }
            }
        }

        let payload = format!(
            "🟧 BLOCK {} 🟧\n{} non-BS OP_RETURN outputs:\n\n{}",
            chaintip.separate_with_commas(),
            op_returns.len(),
            op_returns.join("\n")
        );
        println!("\n{}\n", payload);

        if op_returns.len() == 0 {
            break;
        }

        // poast on x.com
        if POAST_X {
            let x_client = TweetyClient::new(
                &kv["X_CONSUMER_KEY"],
                &kv["X_ACCESS_TOKEN"],
                &kv["X_CONSUMER_SECRET"],
                &kv["X_ACCESS_TOKEN_SECRET"],
            );

            match x_client.post_tweet(&payload, None).await {
                Ok(_) => {
                    println!("successfully poasted to x: https://x.com/opreturn_bot");
                }
                Err(e) => {
                    println!(
                        "error while poasting to https://x.com/opreturn_bot: {:?}",
                        e
                    );
                }
            }
        }

        // poast on nostr
        if POAST_NOSTR {
            let privkey = match Keys::parse(&kv["NOSTR_SEC"]) {
                Ok(key) => key,
                Err(e) => {
                    println!("error while parsing nostr private key: {:#?}", e);
                    break;
                }
            };

            let nostr_client = NostrClient::new(privkey);

            for relay in NOSTR_RELAYS {
                nostr_client.add_relay(*relay).await.unwrap();
                nostr_client.connect().await;
            }

            let builder = EventBuilder::text_note(payload);

            match nostr_client.send_event_builder(builder).await {
                Ok(output) => {
                    println!(
                        "successfully poasted to nostr: https://njump.me/{}",
                        output.id().to_hex()
                    );
                }
                Err(e) => {
                    println!("error while poasting to nostr:\n{:#?}", e);
                }
            }
        }
    }

    Ok(())
}
