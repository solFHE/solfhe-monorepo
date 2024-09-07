// 🏗️ Developed by: Baturalp Güvenç 

/* Gerekli kütüphaneleri kullanıyoruz: rusqlite (SQLite işlemleri için), url (URL ayrıştırma için), serde_json (JSON işlemleri için) ve Rust standart kütüphanesinden çeşitli modüller.
HistoryAnalyzer adında bir struct tanımlıyoruz. Bu struct, linkleri ve kelime sayımlarını tutar.
get_chrome_history_path fonksiyonu, farklı işletim sistemleri için Chrome geçmiş dosyasının konumunu belirler.
extract_links_from_chrome metodu, Chrome'un geçmiş veritabanından son 5 URL'yi çeker.
analyze_link metodu, her bir linki ayrıştırır ve içindeki anlamlı kelimeleri (özellikle blockchain ağı isimlerini) sayar.
get_most_common_word ve to_json metotları, en sık kullanılan kelimeyi bulur ve JSON formatında çıktı üretir.
run metodu, sürekli çalışan bir döngü içinde her 60 saniyede bir yeni linkleri kontrol eder. */

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use serde_json::json;
use rusqlite::Connection;
use url::Url;
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose};
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    system_instruction,
    pubkey::Pubkey,
};
use solana_client::rpc_client::RpcClient;
use spl_memo;

const BLOCKCHAIN_NETWORKS: [&str; 20] = [
    "bitcoin", "ethereum", "scroll", "polkadot", "solana", "avalanche", "cosmos",
    "algorand", "mina", "chainlink", "uniswap", "aave", "compound", "maker",
    "polygon", "binance", "tron", "wormhole", "stellar", "filecoin"
];

const IGNORED_WORDS: [&str; 6] = [
    "http", "https", "www", "com", "org", "net"
];

fn get_chrome_history_path() -> PathBuf {
    let home = dirs::home_dir().expect("Unable to find home directory");
    if cfg!(target_os = "windows") {
        home.join(r"AppData\Local\Google\Chrome\User Data\Default\History")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Google/Chrome/Default/History")
    } else {
        home.join(".config/google-chrome/Default/History")
    }
}

fn extract_links_from_chrome() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let history_path = get_chrome_history_path();
    let temp_path = history_path.with_extension("tmp");

    fs::copy(&history_path, &temp_path)?;

    let conn = Connection::open(&temp_path)?;
    let mut stmt = conn.prepare("SELECT url FROM urls ORDER BY last_visit_time DESC LIMIT 5")?;
    
    let urls: Vec<String> = stmt.query_map([], |row| row.get(0))?
        .filter_map(Result::ok)
        .collect();

    fs::remove_file(temp_path)?;

    Ok(urls)
}

fn extract_keywords_from_url(url: &str) -> Vec<String> {
    let ignored_words: HashSet<_> = IGNORED_WORDS.iter().map(|&s| s.to_string()).collect();
    
    if let Ok(parsed_url) = Url::parse(url) {
        let domain = parsed_url.domain().unwrap_or("");
        let path = parsed_url.path();
        
        domain.split('.')
            .chain(path.split('/'))
            .filter_map(|segment| {
                let lowercase_segment = segment.to_lowercase();
                if segment.is_empty() || ignored_words.contains(&lowercase_segment) {
                    None
                } else {
                    Some(lowercase_segment)
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

fn analyze_link(link: &str, word_counter: &mut HashMap<String, u32>) {
    let keywords = extract_keywords_from_url(link);

    for word in keywords {
        if BLOCKCHAIN_NETWORKS.contains(&word.as_str()) || word.len() > 3 {
            *word_counter.entry(word).or_insert(0) += 1;
        }
    }
}

fn get_most_common_word(word_counter: &HashMap<String, u32>) -> Option<(String, u32)> {
    word_counter.iter()
        .max_by_key(|&(_, count)| count)
        .map(|(word, count)| (word.clone(), *count))
}

fn zk_compress(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    general_purpose::STANDARD_NO_PAD.encode(result)
}

fn zk_decompress(compressed_data: &str) -> Result<String, base64::DecodeError> {
    let bytes = general_purpose::STANDARD_NO_PAD.decode(compressed_data)?;
    Ok(hex::encode(bytes))
}

fn create_solana_account() -> Keypair {
    Keypair::new()
}

fn airdrop_sol(client: &RpcClient, pubkey: &Pubkey, amount: u64) -> Result<(), Box<dyn std::error::Error>> {
    let sig = client.request_airdrop(pubkey, amount)?;
    client.confirm_transaction(&sig)?;
    println!("Airdropped {} lamports", amount);
    
    let balance = client.get_balance(pubkey)?;
    println!("Current balance: {} lamports", balance);
    
    Ok(())
}

fn transfer_compressed_hash(
    client: &RpcClient,
    payer: &Keypair,
    from: &Pubkey,
    to: &Pubkey,
    compressed_hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let balance = client.get_balance(&payer.pubkey())?;
    println!("Current balance before transfer: {} lamports", balance);

    if balance < 5000 {
        return Err("Insufficient balance for transfer".into());
    }

    let transfer_amount = 1000; // Transfer 1000 lamports (approximately 0.000001 SOL)
    let transfer_ix = system_instruction::transfer(from, to, transfer_amount);
    let memo_ix = spl_memo::build_memo(compressed_hash.as_bytes(), &[&payer.pubkey()]);
    
    let recent_blockhash = client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[transfer_ix, memo_ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );
    
    client.send_and_confirm_transaction(&transaction)?;
    println!("Successfully transferred compressed hash");

    let new_balance = client.get_balance(&payer.pubkey())?;
    println!("Current balance after transfer: {} lamports", new_balance);

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Solana localnet
    let client = RpcClient::new("http://localhost:8899".to_string());
    
    let account1 = create_solana_account();
    let account2 = create_solana_account();
    
    println!("Account 1 public key: {}", account1.pubkey());
    println!("Account 2 public key: {}", account2.pubkey());
    
    // Airdrop on localnet
    airdrop_sol(&client, &account1.pubkey(), 1_000_000_000)?;
    
    let mut links = Vec::new();
    let mut word_counter = HashMap::new();

    loop {
        match extract_links_from_chrome() {
            Ok(urls) if !urls.is_empty() => {
                for url in urls {
                    if !links.contains(&url) {
                        links.push(url.clone());
                        analyze_link(&url, &mut word_counter);
                        println!("Analyzed new link: {}", url);

                        if links.len() >= 5 {
                            let result = if let Some((word, count)) = get_most_common_word(&word_counter) {
                                json!({
                                    "most_common_word": word,
                                    "count": count
                                })
                            } else {
                                json!({"error": "No words analyzed yet"})
                            };

                            let json_string = result.to_string();
                            let compressed_result = zk_compress(&json_string);
                            println!("\nSolfhe Result (ZK compressed):");
                            println!("{}", compressed_result);

                            match transfer_compressed_hash(&client, &account1, &account1.pubkey(), &account2.pubkey(), &compressed_result) {
                                Ok(_) => println!("Successfully transferred hash"),
                                Err(e) => println!("Error during hash transfer: {}", e),
                            }

                            match zk_decompress(&compressed_result) {
                                Ok(decompressed_data) => {
                                    println!("\nDecompressed data (hash):");
                                    println!("{}", decompressed_data);
                                },
                                Err(e) => println!("Error decompressing: {}", e),
                            }

                            links.clear();
                            word_counter.clear();
                        }
                    }
                }
            },
            Ok(_) => println!("No new links found"),
            Err(e) => println!("Error extracting links from Chrome: {}", e),
        }
        thread::sleep(Duration::from_secs(60));
    }
}