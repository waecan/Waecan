use waecnan_miner::{build_block_template, mine_block, MinerConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut address = String::from("wae1test");
    let mut threads: usize = 1;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--address" => {
                i += 1;
                if i < args.len() {
                    address = args[i].clone();
                }
            }
            "--threads" => {
                i += 1;
                if i < args.len() {
                    threads = args[i].parse().unwrap_or(1);
                }
            }
            "--help" => {
                println!("Waecnan Miner");
                println!("  --address <wae1...>   Wallet address to receive rewards");
                println!("  --threads <n>         Number of CPU threads (default: 1)");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    println!("Waecnan Miner");
    println!("Address: {}", address);
    println!("Threads: {}", threads);
    println!("Connecting to seed node: 157.173.106.62:19334");
    println!("Mining...");

    let _config = MinerConfig {
        threads,
        miner_address: address.clone(),
    };

    // Use the ED25519 basepoint as a placeholder miner output key
    let miner_key = curve25519_dalek::constants::ED25519_BASEPOINT_POINT.compress();

    let template = build_block_template(
        [0u8; 32],
        1,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        0x2007_FFFFu64,
        vec![],
        miner_key,
    );

    println!("Block template built. Height: {}", template.header.height);
    println!("Mining block 1 on difficulty 0x2007FFFF (testnet low difficulty)...");

    let seed_hash = [0u8; 32];
    let block = mine_block(template, seed_hash);
    println!("Block found! Nonce: {}", block.header.nonce);
    println!("Mining loop would continue here with P2P integration in next phase.");
}
