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
    success: bool,
}

impl RollOnceStratege {
    fn new(is_bull: bool, capital: f64, config: RollConfig) -> Self {
        Self {
            is_bull,
            capital,
            config,
            contract: None,
            success: true,
        }
    }
    fn status(&self) -> Option<(usize, f64)> {
        if self.success {
            Some((self.config.index, self.value()))
        } else {
            None
        }
    }
}

impl Strategy for RollOnceStratege {
    fn update(&mut self, candle: &CandleData) {
        if !self.success {
            return;
        }
        if let Some(contract) = self.contract.take() {
            if let Some(r) = contract.liquidate(if self.is_bull {
                candle.low
            } else {
                candle.high
            }) {
                self.capital += r;
                self.success = false;
                return;
            }
            if (self.is_bull
                && candle.close
                    > contract.entry_price * (1. + self.config.config[self.config.index - 1].1))
                || (!self.is_bull
                    && candle.close
                        < contract.entry_price * (1. - self.config.config[self.config.index - 1].1))
            {
                self.capital += contract.close(candle.close);
            } else {
                self.contract = Some(contract);
            }
        }
        if self.contract.is_some() {
            return;
        }
        if self.config.index >= self.config.config.len() {
            return;
        }
        let leverage = self.config.config[self.config.index].0;
        let contract = Contract::open(
            self.is_bull,
            candle.close,
            self.capital,
            leverage,
            candle.close_time,
            None,
        );
        self.capital = 0.;
        self.contract = Some(contract);
        self.config.index += 1;
        info!(
            "roll once: time: {}, price: {}, status: {:?}",
            candle.close_time,
            candle.close,
            self.status()
        );
    }
    fn close(&mut self, price: f64) -> f64 {
        if let Some(contract) = &self.contract.take() {
            self.capital += contract.close(price);
        }
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
type Increase = f64;
#[derive(Debug, Clone)]
pub struct RollConfig {
    index: usize,

    config: Vec<(Leverage, Increase)>,
}

impl RollConfig {
    pub fn new(config: Vec<(Leverage, Increase)>) -> Self {
        Self {
            index: 0,
            config,
        }
    }
}
impl Default for RollConfig {
    fn default() -> Self {
        let config = vec![
            (25., 0.04),
            (20., 0.05),
            (20., 0.05),
            (15., 0.07),
            (10., 0.1),
            (5., 0.2),
            (3., 0.35),
            (2., 0.5),
            (1., 1.),
            (0.5, 2.),
            (0.2, 5.),
            (0.1, 10.),
        ];
        Self::new(config)
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
            && c.close_time
                < OffsetDateTime::new_utc(
                    Date::from_calendar_date(2021, time::Month::March, 13).unwrap(),
                    Time::from_hms(0, 0, 0).unwrap(),
                )
    });
    let mut strategy = RollOnceStratege::new(true, 100., RollConfig::default());
    for c in chart.candles.iter() {
        strategy.update(&c);
    }
    strategy.close(chart.candles.last().unwrap().close);
    info!(
        "result: {:?}, return rate: {}",
        strategy.status(),
        strategy.value() / 100.
    );
}
