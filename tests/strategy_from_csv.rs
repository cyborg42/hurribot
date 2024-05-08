#![allow(dead_code)]
use hurribot::{
    backtest::{
        candle_chart::CandleChart,
        strategy::{geo_strategy::GeoStrategy, Strategy},
    },
    init_log, local_now,
};
use std::sync::{Arc, Mutex};
use time::Duration;
use tracing::info;

// TODO: 币安会在0:00 8:00 16:00进行资金费率结算，若需支付资金费率则提前一分钟平仓并推迟10s建仓，若需收取资金费率则推迟一分钟平仓+建仓

#[test]
fn strategy_from_csv() {
    let log_name = local_now()
        .format(
            &time::format_description::parse(
                "hurribot_[year]-[month]-[day]T[hour]:[minute]:[second]",
            )
            .unwrap(),
        )
        .unwrap();
    let _logger_guard = init_log(&log_name);
    let chart = CandleChart::read_from_csv("./data/BTCUSDT", Duration::minutes(1));
    let total_capital = Arc::new(Mutex::new(1000000.));
    let ratio = 1.;
    let leverage = 10.;
    let mut strategy = GeoStrategy::new(
        true,
        leverage,
        ratio,
        Duration::minutes(60),
        10.,
        0.03,
        0.002,
        total_capital.clone(),
    );

    for (i, candle) in chart.candles.iter().enumerate() {
        if i % 4000 == 0 {
            info!(
                "round: {}, price: {}, value: {}, return rate: {}",
                candle.close_time,
                candle.close,
                strategy.value(),
                strategy.value() / strategy.cost
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
