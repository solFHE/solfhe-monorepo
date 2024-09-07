// Baturalp Güvenç
/* Gerekli kütüphaneleri kullanıyoruz: rusqlite (SQLite işlemleri için), url (URL ayrıştırma için), serde_json (JSON işlemleri için) ve Rust standart kütüphanesinden çeşitli modüller.
HistoryAnalyzer adında bir struct tanımlıyoruz. Bu struct, linkleri ve kelime sayımlarını tutar.
get_chrome_history_path fonksiyonu, farklı işletim sistemleri için Chrome geçmiş dosyasının konumunu belirler.
extract_links_from_chrome metodu, Chrome'un geçmiş veritabanından son 5 URL'yi çeker.
analyze_link metodu, her bir linki ayrıştırır ve içindeki anlamlı kelimeleri (özellikle blockchain ağı isimlerini) sayar.
get_most_common_word ve to_json metotları, en sık kullanılan kelimeyi bulur ve JSON formatında çıktı üretir.
run metodu, sürekli çalışan bir döngü içinde her 60 saniyede bir yeni linkleri kontrol eder. */
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

const BLOCKCHAIN_NETWORKS: [&str; 20] = [
    "bitcoin", "ethereum", "scroll", "polkadot", "solana", "avalanche", "cosmos",
    "algorand", "mina", "chainlink", "uniswap", "aave", "compound", "maker",
    "polygon", "binance", "tron", "wormhole", "stellar", "filecoin"
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

fn analyze_link(link: &str, word_counter: &mut HashMap<String, u32>) {
    let words: Vec<&str> = link.split(|c: char| !c.is_alphanumeric())
        .filter(|&word| !word.is_empty())
        .collect();

    for word in words {
        let word = word.to_lowercase();
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

fn main() {
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
                            if let Some((word, count)) = get_most_common_word(&word_counter) {
                                println!("\nSolfhe Result:");
                                println!("{{\"{}\":{}}}", word, count);
                            } else {
                                println!("{{\"error\":\"No words analyzed yet\"}}");
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