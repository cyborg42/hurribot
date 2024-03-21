use std::{sync::Arc, thread::sleep, time::Duration};

use crossbeam::channel::unbounded;
use hurribot::{
    algrithm,
    binance_futures::FuturesWsConnection,
    market::{self, SymbolPriceInfo},
    stdout_logger,
};
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
    use binance::futures::websockets::*;
    use std::sync::atomic::AtomicBool;
    let running = Arc::new(AtomicBool::new(true)); // Used to control the event loop
    let (symbol_tx, symbol_rx) = unbounded();
    let handler = move |event: FuturesWebsocketEvent| {
        match event {
            FuturesWebsocketEvent::MarkPriceAll(v) => {
                let m: Vec<_> = v
                    .into_iter()
                    .map(|p| {
                        let symbol = SymbolPriceInfo {
                            price: p.mark_price.parse().unwrap_or_default(),
                            update_time: p.event_time,
                            funding_rate: p.funding_rate.parse().unwrap_or_default(),
                        };
                        (p.symbol, symbol)
                    })
                    .collect();
                symbol_tx.send(m).unwrap();
            }
            _ => {}
        }
        Ok(())
    };

    let subscribes = vec!["!markPrice@arr".to_string()];

    let conn = FuturesWsConnection::MarketData(subscribes);
    conn.run(handler, running.clone());

    std::thread::spawn(move || {
        let binance_config =
            hurribot::binance_futures::BinanceConfig::value_parse("./config/binance_config.toml")
                .unwrap();

        let mut market: market::MarketStatus<algrithm::roll::RollAlgrithm> =
            market::MarketStatus::new(binance_config).unwrap();

        for symbols in symbol_rx {
            for (symbol, status) in symbols {
                market.update(symbol, status)
            }
            println!("{:#?}", market);
        }
    });
    sleep(Duration::from_secs(60));
}
