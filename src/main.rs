use hurribot::{
    binance_futures::{BinanceKeys, FuturesWsConnection},
    market,
    utils::stdout_logger,
};
use tracing::info;
fn main() {
    // let _guard = file_logger("main");
    stdout_logger();
    info!("start");
    let (price_rx, prices, conn_h) = FuturesWsConnection::run_price_info();
    let binance_keys = BinanceKeys::value_parse("./config/binance_keys.toml").unwrap();

    conn_h.join().unwrap();
}
