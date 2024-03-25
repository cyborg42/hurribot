use hurribot::{
    algorithm::{roll::RollAlgrithm, AlgorithmsManager},
    binance_futures::{BinanceConfig, FuturesWsConnection},
    market, stdout_logger,
};
use tracing::info;
fn main() {
    // let _ = file_logger("main");
    stdout_logger();
    info!("start");
    let (price_rx, conn_h) = FuturesWsConnection::run_price_info();
    let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
    let (signal_rx, _) = AlgorithmsManager::new((RollAlgrithm::new(),)).run(price_rx);

    for s in signal_rx.iter() {
        info!("{:?}", s);
    }
    conn_h.join().unwrap();
}
