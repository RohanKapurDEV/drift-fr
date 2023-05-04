use anchor_client::anchor_lang::AccountDeserialize;
use dotenv::dotenv;
use drift::{
    math::{casting::Cast, constants::PRICE_PRECISION, safe_math::SafeMath},
    state::{oracle::OraclePriceData, perp_market::PerpMarket},
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::{
    error::Error,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

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

pub async fn fetch_pyth_price2(
    price_account_address: &Pubkey,
    clock_slot: u64,
    multiple: u128,
    rpc_url: &String,
) -> Result<OraclePriceData, Box<dyn Error>> {
    let client = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());

    let price_account_data = client.get_account(price_account_address).await?.data;
    let price_account = pyth_sdk_solana::state::load_price_account(&price_account_data)?;

    let oracle_price = price_account.agg.price;
    let oracle_conf = price_account.agg.conf;

    let oracle_precision = 10_u128.pow(price_account.expo.unsigned_abs());

    if oracle_precision <= multiple {
        println!("Multiple larger than oracle precision");
        return Err("Multiple larger than oracle precision".into());
    }

    let oracle_precision = oracle_precision.safe_div(multiple).unwrap();

    let mut oracle_scale_mult = 1;
    let mut oracle_scale_div = 1;

    if oracle_precision > PRICE_PRECISION {
        oracle_scale_div = oracle_precision.safe_div(PRICE_PRECISION).unwrap();
    } else {
        oracle_scale_mult = PRICE_PRECISION.safe_div(oracle_precision).unwrap();
    }

    let oracle_price_scaled = (oracle_price)
        .cast::<i128>()
        .unwrap()
        .safe_mul(oracle_scale_mult.cast().unwrap())
        .unwrap()
        .safe_div(oracle_scale_div.cast().unwrap())
        .unwrap()
        .cast::<i64>()
        .unwrap();

    let oracle_conf_scaled = (oracle_conf)
        .cast::<u128>()
        .unwrap()
        .safe_mul(oracle_scale_mult)
        .unwrap()
        .safe_div(oracle_scale_div)
        .unwrap()
        .cast::<u64>()
        .unwrap();

    let oracle_delay: i64 = clock_slot
        .cast::<i64>()
        .unwrap()
        .safe_sub(price_account.valid_slot.cast().unwrap())
        .unwrap();

    Ok(OraclePriceData {
        price: oracle_price_scaled,
        confidence: oracle_conf_scaled,
        delay: oracle_delay,
        has_sufficient_number_of_data_points: true,
    })
}
