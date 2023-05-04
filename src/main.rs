use anchor_client::anchor_lang::AccountDeserialize;
use dotenv::dotenv;
use drift::state::perp_market::PerpMarket;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::{error::Error, str::FromStr};

#[tokio::main]
async fn main() {
    dotenv().ok();

    let sol_perp_market_key =
        Pubkey::from_str("8UJgxaiQx5nTrdDgph5FiahMmzduuLTLf5WmsPegYA6W").unwrap();

    let rpc_url = std::env::var("RPC_URL").unwrap();

    let perp_market_account = fetch_perp_market_account(&sol_perp_market_key, &rpc_url)
        .await
        .unwrap();
    let mp_twap = perp_market_account.amm.last_mark_price_twap as f64 / 1e6;
    let oracle_twap = perp_market_account
        .amm
        .historical_oracle_data
        .last_oracle_price_twap as f64
        / 1e6;

    let mp_twap_ts = perp_market_account.amm.last_mark_price_twap_ts;
    let oracle_twap_ts = perp_market_account
        .amm
        .historical_oracle_data
        .last_oracle_price_twap_ts;

    let fr = (1.0 / 24.0) * ((mp_twap - oracle_twap) / oracle_twap);
    let fr_pct = fr * 100.0;

    println!("Funding rate percentage: {}", fr_pct);

    println!("mp_twap: {}", mp_twap as f64 / 1e6);
    println!("oracle_twap: {}", oracle_twap as f64 / 1e6);

    println!("mp_twap_ts: {}", mp_twap_ts);
    println!("oracle_twap_ts: {}", oracle_twap_ts);
}

/// Fetch a Perp Market account from Drift
pub async fn fetch_perp_market_account(
    address: &Pubkey,
    rpc_url: &String,
) -> Result<PerpMarket, Box<dyn Error>> {
    let client = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());

    let perp_market_account_data = client.get_account(address).await?.data;
    let perp_market_account = PerpMarket::try_deserialize(&mut &perp_market_account_data[..])?;

    Ok(perp_market_account)
}
