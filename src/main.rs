use std::{sync::Arc, thread::sleep, time::Duration};

use binance::futures::websockets::*;
use crossbeam::channel::unbounded;
use hurribot::{
    algorithm::{roll::RollAlgrithm, AlgorithmsManager}, binance_futures::{BinanceConfig, FuturesWsConnection}, market::{self, SymbolPriceInfo}, stdout_logger
};
use std::sync::atomic::AtomicBool;
use tracing::info;
fn main() {
    // let log_name = hurribot::local_now()
    //     .format(
    //         &time::format_description::parse(
    //             "hurribot_[year]-[month]-[day]T[hour]:[minute]:[second]",
    //         )
    //         .unwrap(),
    //     )
    //     .unwrap();
    // let _logger_guard = hurribot::init_log(&log_name);

    stdout_logger();
    info!("start");
    let running = Arc::new(AtomicBool::new(true)); // Used to control the event loop
    let (symbol_tx, symbol_rx) = unbounded();
    let handler = move |event: FuturesWebsocketEvent| {
        match event {
            FuturesWebsocketEvent::MarkPriceAll(v) => {
                v.into_iter().for_each(|p| {
                    let s = SymbolPriceInfo {
                        price: p.mark_price.parse().unwrap_or_default(),
                        update_time: p.event_time,
                        funding_rate: p.funding_rate.parse().unwrap_or_default(),
                    };
                    symbol_tx.send((p.symbol, s)).unwrap();
                });
            }
            _ => {}
        }
        Ok(())
    };

    let subscribes = vec!["!markPrice@arr".to_string()];

    let conn = FuturesWsConnection::MarketData(subscribes);
    conn.run(handler, running.clone());
    let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

    let _market: market::MarketStatus = market::MarketStatus::new(binance_config).unwrap();
    let (signal_tx, signal_rx) = unbounded();
    std::thread::spawn(move || {
        let mut alg_manager = AlgorithmsManager::new((RollAlgrithm::new(),), signal_tx);
        for s in symbol_rx.iter() {
            alg_manager.update(s.0, s.1);
        }
    });
    std::thread::spawn(move || {
        for s in signal_rx.iter() {
            info!("{:?}", s);
        }
    });
    sleep(Duration::from_secs(60));
}
