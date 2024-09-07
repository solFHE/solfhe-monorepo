use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use anchor_client::solana_sdk::signer::keypair::Keypair;
use anchor_client::{Client, Cluster};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tfhe::prelude::*;

const BLOCKCHAIN_NETWORKS: [&str; 20] = [
    "bitcoin", "ethereum", "scroll", "polkadot", "solana", "avalanche", "cosmos",
    "algorand", "mina", "chainlink", "uniswap", "aave", "compound", "maker",
    "polygon", "binance", "tron", "wormhole", "stellar", "filecoin"
];

struct HistoryAnalyzer {
    links: Vec<String>,
    word_counter: HashMap<String, u32>,
}

impl HistoryAnalyzer {
    fn new() -> Self {
        HistoryAnalyzer {
            links: Vec::new(),
            word_counter: HashMap::new(),
        }
    }

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
        let history_path = Self::get_chrome_history_path();
        let temp_path = history_path.with_extension("tmp");

        fs::copy(&history_path, &temp_path).expect("Failed to copy history file");

        let conn = rusqlite::Connection::open(&temp_path).expect("Failed to open database");
        let mut stmt = conn.prepare("SELECT url FROM urls ORDER BY last_visit_time DESC LIMIT 5")
            .expect("Failed to prepare statement");
        
        let urls: Vec<String> = stmt.query_map([], |row| row.get(0))
            .expect("Failed to execute query")
            .filter_map(Result::ok)
            .collect();

        fs::remove_file(temp_path).expect("Failed to remove temporary file");

        urls
    }

    fn analyze_link(&mut self, link: &str) {
        let words: Vec<&str> = link.split(|c: char| !c.is_alphanumeric())
            .filter(|&word| !word.is_empty())
            .collect();

        for word in words {
            let word = word.to_lowercase();
            if BLOCKCHAIN_NETWORKS.contains(&word.as_str()) || word.len() > 3 {
                *self.word_counter.entry(word).or_insert(0) += 1;
            }
        }
    }

    fn get_most_common_word(&self) -> Option<(String, u32)> {
        self.word_counter.iter()
            .max_by_key(|&(_, count)| count)
            .map(|(word, count)| (word.clone(), *count))
    }

    fn to_json(&self) -> String {
        if let Some((word, count)) = self.get_most_common_word() {
            serde_json::json!({ word: count }).to_string()
        } else {
            serde_json::json!({ "error": "No words analyzed yet" }).to_string()
        }
    }

    fn encrypt_data(&self, client_key: &ClientKey) -> Vec<u8> {
        let json_data = self.to_json();
        let encrypted_data = client_key.encrypt(json_data.as_bytes());
        bincode::serialize(&encrypted_data).unwrap()
    }

    fn decrypt_data(encrypted_data: &[u8], client_key: &ClientKey) -> String {
        let deserialized: Vec<u8> = bincode::deserialize(encrypted_data).unwrap();
        let decrypted_data = client_key.decrypt(&deserialized);
        String::from_utf8(decrypted_data).unwrap()
    }

    fn send_to_solana(client: &Client, program_id: &Pubkey, ad_account: &Pubkey, encrypted_data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let instruction = client
            .program(*program_id)
            .request()
            .accounts(fhe_solana_ad::accounts::StoreEncryptedData {
                ad_account: *ad_account,
                authority: client.payer(),
            })
            .args(fhe_solana_ad::instruction::StoreEncryptedData {
                encrypted_data: encrypted_data.to_vec(),
            })
            .instructions()?;

        let signature = client.send_and_confirm(&instruction, &[client.payer()])?;
        println!("Transaction sent: {}", signature);
        Ok(())
    }

    fn get_ad_link(keyword: &str) -> String {
        format!("https://example.com/ads/{}", keyword)
    }

    fn run(&mut self, client: &Client, program_id: &Pubkey, ad_account: &Pubkey) {
        let (client_key, _) = generate_keys();

        loop {
            match Self::extract_links_from_chrome() {
                urls if !urls.is_empty() => {
                    for url in urls {
                        if !self.links.contains(&url) {
                            self.links.push(url.clone());
                            self.analyze_link(&url);
                            println!("Analyzed new link: {}", url);

                            if self.links.len() >= 5 {
                                let encrypted_data = self.encrypt_data(&client_key);
                                if let Err(e) = Self::send_to_solana(client, program_id, ad_account, &encrypted_data) {
                                    eprintln!("Error sending data to Solana: {}", e);
                                } else {
                                    println!("Data sent to Solana successfully");
                                }

                                let decrypted_data = Self::decrypt_data(&encrypted_data, &client_key);
                                println!("Decrypted data: {}", decrypted_data);

                                if let Some((word, _)) = self.get_most_common_word() {
                                    let ad_link = Self::get_ad_link(&word);
                                    println!("Relevant ad link: {}", ad_link);
                                }

                                self.links.clear();
                                self.word_counter.clear();
                            }
                        }
                    }
                },
                _ => println!("No new links found"),
            }
            thread::sleep(Duration::from_secs(60));
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let payer = Keypair::new();
    let client = Client::new_with_options(
        Cluster::Devnet,
        Rc::new(payer),
        CommitmentConfig::confirmed(),
    );

    let program_id = Pubkey::new_unique(); // Bu değeri gerçek program ID'nizle değiştirin
    let ad_account = Pubkey::new_unique(); // Bu değeri gerçek hesap adresinizle değiştirin

    let mut analyzer = HistoryAnalyzer::new();
    analyzer.run(&client, &program_id, &ad_account);

    Ok(())
}