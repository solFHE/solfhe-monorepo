// Baturalp Güvenç
/* Gerekli kütüphaneleri kullanıyoruz: rusqlite (SQLite işlemleri için), url (URL ayrıştırma için), serde_json (JSON işlemleri için) ve Rust standart kütüphanesinden çeşitli modüller.
HistoryAnalyzer adında bir struct tanımlıyoruz. Bu struct, linkleri ve kelime sayımlarını tutar.
get_chrome_history_path fonksiyonu, farklı işletim sistemleri için Chrome geçmiş dosyasının konumunu belirler.
extract_links_from_chrome metodu, Chrome'un geçmiş veritabanından son 5 URL'yi çeker.
analyze_link metodu, her bir linki ayrıştırır ve içindeki anlamlı kelimeleri (özellikle blockchain ağı isimlerini) sayar.
get_most_common_word ve to_json metotları, en sık kullanılan kelimeyi bulur ve JSON formatında çıktı üretir.
run metodu, sürekli çalışan bir döngü içinde her 60 saniyede bir yeni linkleri kontrol eder. */
// Baturalp Güvenç
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
use spl_token::native_mint;
use spl_associated_token_account::instruction as spl_associated_token_instruction;
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

fn extract_links_from_chrome() -> Vec<String> {
    let history_path = get_chrome_history_path();
    let temp_path = history_path.with_extension("tmp");

    fs::copy(&history_path, &temp_path).expect("Failed to copy history file");

    let conn = Connection::open(&temp_path).expect("Failed to open database");
    let mut stmt = conn.prepare("SELECT url FROM urls ORDER BY last_visit_time DESC LIMIT 5")
        .expect("Failed to prepare statement");
    
    let urls: Vec<String> = stmt.query_map([], |row| row.get(0))
        .expect("Failed to execute query")
        .filter_map(Result::ok)
        .collect();

    fs::remove_file(temp_path).expect("Failed to remove temporary file");

    urls
}

fn extract_keywords_from_url(url: &str) -> Vec<String> {
    let ignored_words: HashSet<_> = IGNORED_WORDS.iter().map(|&s| s.to_string()).collect();
    
    if let Ok(parsed_url) = Url::parse(url) {
        let domain = parsed_url.domain().unwrap_or("");
        let path = parsed_url.path();
        
        let keywords: Vec<String> = domain.split('.')
            .chain(path.split('/'))
            .filter_map(|segment| {
                if segment.is_empty() || ignored_words.contains(segment.to_lowercase().as_str()) {
                    None
                } else {
                    Some(segment.to_lowercase())
                }
            })
            .collect();
        
        keywords
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

// Temsili ZK compression fonksiyonu
fn zk_compress(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    general_purpose::STANDARD_NO_PAD.encode(result)
}

// Temsili ZK decompression fonksiyonu
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
    Ok(())
}

fn airdrop_sol_with_retry(client: &RpcClient, pubkey: &Pubkey, total_amount: u64, max_retries: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut remaining_amount = total_amount;
    let mut retries = 0;

    while remaining_amount > 0 && retries < max_retries {
        let request_amount = remaining_amount.min(1_000_000_000); // Request max 1 SOL at a time
        match client.request_airdrop(pubkey, request_amount) {
            Ok(sig) => {
                match client.confirm_transaction(&sig) {
                    Ok(_) => {
                        println!("Airdropped {} lamports", request_amount);
                        remaining_amount -= request_amount;
                        retries = 0; // Resetle
                    },
                    Err(e) => {
                        println!("Error confirming transaction: {}", e);
                        retries += 1;
                    }
                }
            },
            Err(e) => {
                println!("Error requesting airdrop: {}", e);
                retries += 1;
            }
        }
        thread::sleep(Duration::from_secs(1)); // Bekle
    }

    if remaining_amount == 0 {
        Ok(())
    } else {
        Err("Failed to airdrop the full amount after multiple retries".into())
    }
}

fn create_spl_token_account(
    client: &RpcClient,
    payer: &Keypair,
    token_mint: &Pubkey,
    owner: &Pubkey,
) -> Result<Pubkey, Box<dyn std::error::Error>> {
    let associated_token_address = spl_associated_token_account::get_associated_token_address(owner, token_mint);
    
    let create_ata_ix = spl_associated_token_instruction::create_associated_token_account(
        &payer.pubkey(),
        owner,
        token_mint,
        &spl_token::id(),
    );
    
    let recent_blockhash = client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[create_ata_ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );
    
    client.send_and_confirm_transaction(&transaction)?;
    
    Ok(associated_token_address)
}

fn transfer_compressed_hash(
    client: &RpcClient,
    payer: &Keypair,
    from: &Pubkey,
    to: &Pubkey,
    compressed_hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let transfer_ix = system_instruction::transfer(from, to, 1); // 1 lamport transfer
    let memo_ix = spl_memo::build_memo(compressed_hash.as_bytes(), &[&payer.pubkey()]);
    
    let recent_blockhash = client.get_latest_blockhash()?;
    let transaction = Transaction::new_signed_with_payer(
        &[transfer_ix, memo_ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );
    
    client.send_and_confirm_transaction(&transaction)?;
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RpcClient::new("https://api.devnet.solana.com".to_string());
    
    // İki hesap oluştur
    let account1 = create_solana_account();
    let account2 = create_solana_account();
    
    println!("Account 1 public key: {}", account1.pubkey());
    println!("Account 2 public key: {}", account2.pubkey());
    

    
    // Account 1'e SOL airdrop
    match airdrop_sol_with_retry(&client, &account1.pubkey(), 1_000_000_000, 5) {
        Ok(_) => println!("Successfully airdropped SOL to Account 1"),
        Err(e) => println!("Failed to airdrop SOL: {}", e),
    }
    
    // SPL token hesabı oluştur (Native SOL için)
    match create_spl_token_account(&client, &account1, &native_mint::id(), &account1.pubkey()) {
        Ok(token_account1) => println!("SPL Token account for Account 1: {}", token_account1),
        Err(e) => println!("Failed to create SPL token account: {}", e),
    }    
    
    let mut links = Vec::new();
    let mut word_counter = HashMap::new();

    loop {
        match extract_links_from_chrome() {
            urls if !urls.is_empty() => {
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

                            // ZK compressed hash'i transfer et
                            transfer_compressed_hash(&client, &account1, &account1.pubkey(), &account2.pubkey(), &compressed_result)?;
                            println!("Transferred compressed hash from Account 1 to Account 2");

                            // ZK compressed sonucu çöz ve göster
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
            _ => println!("No new links found"),
        }
        thread::sleep(Duration::from_secs(60));
    }
}