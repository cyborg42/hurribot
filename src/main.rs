#![allow(dead_code)]
use hurribot::{candle_chart::{CandleChart, CandleData}, strategy::GeoStrategy, init_log};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};
use hurribot::contract::{Contract, HANDLING_FEE_RATE_MAKER, HANDLING_FEE_RATE_TAKER};



// TODO: 币安会在0:00 8:00 16:00进行资金费率结算，若需支付资金费率则提前一分钟平仓并推迟10s建仓，若需收取资金费率则推迟一分钟平仓+建仓

fn main() {
    let log_name = chrono::Local::now()
        .format("hurribot_%Y-%m-%d-%H:%M:%S")
        .to_string();
    let _logger_guard = init_log(&log_name);
    let chart = CandleChart::read_from_csv("./data/SOLUSDT", 3600);

    let total_capital = Arc::new(Mutex::new(1000.));
    // chart.candles = chart.candles.into_iter().filter(|c|c.time > 1678996800).collect();
    let ratio = 1.;
    let leverage = 10.;
    // 理论上其他参数固定的情况下，ratio * leverage / (1. + leverage * handling_fee_rate) 相等时，收益率和风险一致
    let mut strategy = GeoStrategy::new(
        leverage,
        ratio,
        3600,
        10.,
        0.97,
        1.002,
        total_capital.clone(),
    );

    for (i, candle) in chart.candles.iter().enumerate() {
        if i % 1000 == 0 {
            info!(
                "round: {i}, price: {}, current value: {}",
                candle.close,
                strategy.value(),
            );
        }
        strategy.update(candle);
    }
    strategy.close(chart.candles.last().unwrap().close);
    let ret = strategy.value() / strategy.cost;
    info!(
        "ratio: {ratio}, leverage: {leverage}, add money: {}, captial: {}, return rate: {}, open count: {}",
        strategy.cost, strategy.value(), ret, strategy.open_count
    );
}


#[test]
fn offer_test() {
    let offer = Contract::open(100., 100., 100., 100, Some(99.9));
    println!("{:?}", offer);
    println!("{:?}", offer.liquidate(9.));
}
