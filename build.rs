// safe_pump/build.rs
// Generates MOTHERSHIP_PROGRAM_ID, SEED_COIN_ID, INTERFACE_PROGRAM_ID at compile time
// Used by all programs and CPI clients

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=../Anchor.toml");

    let anchor_toml = std::fs::read_to_string("../Anchor.toml")
        .expect("Failed to read ../Anchor.toml — run from safe_pump/ or fix path");

    let mut mothership_id = None;
    let mut seed_coin_id = None;
    let mut interface_id = None;

    for line in anchor_toml.lines() {
        if line.contains("safe_pump = \"") {
            mothership_id = line.split('"').nth(1).map(str::to_string);
        }
        if line.contains("seed_coin = \"") {
            seed_coin_id = line.split('"').nth(1).map(str::to_string);
        }
        if line.contains("safe_pump_interface = \"") || line.contains("interface = \"") {
            interface_id = line.split('"').nth(1).map(str::to_string);
        }
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_program_ids.rs");
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&dest_path)
        .expect("Failed to create generated_program_ids.rs");

    writeln!(
        f,
        "pub const MOTHERSHIP_PROGRAM_ID: &str = \"{}\";",
        mothership_id.expect("safe_pump not found in Anchor.toml — run `anchor keys sync`")
    ).unwrap();

    writeln!(
        f,
        "pub const SEED_COIN_ID: &str = \"{}\";",
        seed_coin_id.expect("seed_coin not found in Anchor.toml — run `anchor keys sync`")
    ).unwrap();

    writeln!(
        f,
        "pub const INTERFACE_PROGRAM_ID: &str = \"{}\";",
        interface_id.expect("interface / safe_pump_interface not found in Anchor.toml — run `anchor keys sync`")
    ).unwrap();

    writeln!(f, "pub const MOTHERSHIP_PUBKEY: solana_program::pubkey::Pubkey = solana_program::pubkey!({});", mothership_id.unwrap()).unwrap();
    writeln!(f, "pub const SEED_COIN_PUBKEY: solana_program::pubkey::Pubkey = solana_program::pubkey!({});", seed_coin_id.unwrap()).unwrap();
    writeln!(f, "pub const INTERFACE_PUBKEY: solana_program::pubkey::Pubkey = solana_program::pubkey!({});", interface_id.unwrap()).unwrap();

    println!("cargo:warning=Generated program IDs: MOTHERSHIP={}, SEED_COIN={}, INTERFACE={}", 
        mothership_id.unwrap(), seed_coin_id.unwrap(), interface_id.unwrap());
}
