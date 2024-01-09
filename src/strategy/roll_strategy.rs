use super::Strategy;
use crate::{
    candle_chart::CandleData,
    contract::{Contract, HANDLING_FEE_RATE_MAKER, HANDLING_FEE_RATE_TAKER},
};
use time::{
    macros::{date, datetime},
    Date, Time,
};
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct RollOnceStratege {
    is_bull: bool,
    capital: f64,
    config: RollConfig,
    contract: Option<Contract>,
    level: usize,
    pub max_value: f64,
    pub best_price: f64,
    pub status: RollOnceStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RollOnceStatus {
    Processing,
    Successed,
    Failed,
    Aborted,
}

impl RollOnceStratege {
    fn new(is_bull: bool, capital: f64, config: RollConfig) -> Self {
        Self {
            is_bull,
            capital,
            config,
            contract: None,
            level: 0,
            max_value: 0.,
            best_price: 0.,
            status: RollOnceStatus::Processing,
        }
    }
}

impl Strategy for RollOnceStratege {
    fn update(&mut self, candle: &CandleData) {
        if self.status != RollOnceStatus::Processing {
            return;
        }
        if let Some(contract) = self.contract.take() {
            let (_leverage, take_profit, max_draw) = self.config.0[self.level - 1];
            if let Some(r) = contract.liquidate(if self.is_bull {
                candle.low
            } else {
                candle.high
            }) {
                self.capital += r;
                self.status = RollOnceStatus::Failed;
                info!(
                    "roll once failed: time: {}, price: {}, level: {}, value: {}",
                    candle.close_time,
                    candle.close,
                    self.level,
                    self.value()
                );
                return;
            }
            if (self.is_bull && candle.close > contract.entry_price * (1. + take_profit))
                || (!self.is_bull && candle.close < contract.entry_price * (1. - take_profit))
            {
                self.capital += contract.close(candle.close);
            } else {
                let value_high = contract.close(candle.high);
                let value_low = contract.close(candle.low);
                if self.max_value < value_high {
                    self.max_value = value_high;
                    self.best_price = candle.high;
                }
                if self.max_value < value_low {
                    self.max_value = value_low;
                    self.best_price = candle.low;
                }
                if let Some(max_draw) = max_draw {
                    if contract.close(candle.close) < self.max_value * (1. - max_draw) {
                        self.capital += contract.close(candle.close);
                        self.status = RollOnceStatus::Successed;
                        info!(
                            "roll once successed: time: {}, price: {}, level:, {}, value: {}",
                            candle.close_time,
                            candle.close,
                            self.level,
                            self.value()
                        );
                        return;
                    }
                }
                self.contract = Some(contract);
            }
        }
        if self.contract.is_some() {
            return;
        }
        if self.level >= self.config.0.len() {
            return;
        }
        let leverage = self.config.0[self.level].0;
        let stop_loss = if self.is_bull {
            candle.close * (1. - 0.99 / leverage) + candle.close * 0.004
        } else {
            candle.close * (1. + 0.99 / leverage) - candle.close * 0.004
        };
        let contract = Contract::open(
            self.is_bull,
            candle.close,
            self.capital,
            leverage,
            candle.close_time,
            Some(stop_loss),
        );
        self.capital = 0.;
        self.contract = Some(contract);
        self.level += 1;
        info!(
            "roll once open new: time: {}, price: {}, level: {}, leverage: {}, value: {}",
            candle.close_time,
            candle.close,
            self.level,
            leverage,
            self.value()
        );
    }
    fn close(&mut self, price: f64) -> f64 {
        if let Some(contract) = &self.contract.take() {
            self.capital += contract.close(price);
        }
        self.status = RollOnceStatus::Aborted;
        self.capital
    }
    fn value(&self) -> f64 {
        self.capital
            + if let Some(contract) = &self.contract {
                contract.margin
            } else {
                0.
            }
    }
}

type Leverage = f64;
type TakeProfit = f64;
type MaxDraw = Option<f64>;
#[derive(Debug, Clone)]
struct RollConfig(Vec<(Leverage, TakeProfit, MaxDraw)>);

impl RollConfig {
    pub fn new(config: Vec<(Leverage, TakeProfit, MaxDraw)>) -> Self {
        Self(config)
    }
}
impl Default for RollConfig {
    fn default() -> Self {
        let config = vec![
            (25., 0.04, None),
            (20., 0.05, None),
            (20., 0.05, None),
            (15., 0.07, None),
            (10., 0.1, None),
            (5., 0.2, None),
            (3., 0.35, None),
            (2., 0.5, Some(0.6)),
            (1., 1., Some(0.2)),
        ];
        Self(config)
    }
}

#[test]
fn roll_test() {
    use crate::candle_chart::CandleChart;
    use crate::init_log;
    use crate::local_now;
    use time::Duration;
    use time::OffsetDateTime;

    let log_name = local_now()
        .format(
            &time::format_description::parse(
                "hurribot_roll_[year]-[month]-[day]T[hour]:[minute]:[second]",
            )
            .unwrap(),
        )
        .unwrap();
    let _logger_guard = init_log(&log_name);
    let mut chart = CandleChart::read_from_csv("./data/BTCUSDT", Duration::minutes(1));
    chart.candles.retain(|c| {
        c.close_time
            > OffsetDateTime::new_utc(
                Date::from_calendar_date(2020, time::Month::December, 16).unwrap(),
                Time::from_hms(0, 0, 0).unwrap(),
            )
    });
    let mut strategy = RollOnceStratege::new(true, 100., RollConfig::default());
    for c in chart.candles.iter() {
        strategy.update(&c);
        if strategy.status != RollOnceStatus::Processing {
            break;
        }
    }
    if strategy.status == RollOnceStatus::Processing {
        strategy.close(chart.candles.last().unwrap().close);
    }
    info!(
        "result: {:?}, level: {}, return rate: {}, max return rate: {}, best price: {}",
        strategy.status,
        strategy.level,
        strategy.value() / 100.,
        strategy.max_value / 100.,
        strategy.best_price
    );
}
