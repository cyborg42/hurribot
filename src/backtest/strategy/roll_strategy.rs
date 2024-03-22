use std::collections::VecDeque;

use crate::backtest::{candle_chart::CandleData, contract::Contract};

use super::Strategy;

use tracing::info;

#[derive(Debug, Clone)]
pub struct RollOnceStrategy {
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

impl RollOnceStrategy {
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

impl Strategy for RollOnceStrategy {
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
    fn linear(k: f64, b: f64, max: f64, _step: f64) -> Self {
        let mut config = Vec::new();
        let mut x = 1.;
        while x < max {
            config.push((x, 1., None));
            x = x * k + b;
        }
        Self(config)
    }
}

impl Default for RollConfig {
    fn default() -> Self {
        let config = vec![
            (25., 0.04, None),    // 4%     104%
            (20., 0.05, None),    // 5%     109.2%
            (20., 0.05, None),    // 5%     114.7%
            (15., 0.67, None),    // 6.7%   122.3%
            (10., 0.1, None),     // 10%    134.5%
            (5., 0.2, None),      // 20%    161.4%
            (3., 0.33, None),     // 33%    215.3%
            (2., 0.5, Some(0.6)), // 50%    322.9%
            (1., 1., Some(0.2)),  // 100%   645.7%
        ];
        Self(config)
    }
}

struct RollJudge {
    cache: VecDeque<CandleData>,
    max_length: usize,
}

impl RollJudge {
    fn new(max_length: usize) -> Self {
        let cache = VecDeque::with_capacity(max_length);
        Self { cache, max_length }
    }
    fn update(&mut self, candle: &CandleData) {
        if self.cache.len() >= self.max_length {
            self.cache.pop_back();
        }
        self.cache.push_front(candle.clone());
    }
    fn max(&self, size: usize) -> CandleData {
        self.cache
            .iter()
            .take(size)
            .max_by(|x, y| x.high.partial_cmp(&y.high).unwrap())
            .unwrap()
            .clone()
    }
    fn min(&self, size: usize) -> CandleData {
        self.cache
            .iter()
            .take(size)
            .min_by(|x, y| x.low.partial_cmp(&y.low).unwrap())
            .unwrap()
            .clone()
    }
    fn is_max(&self, size: usize) -> bool {
        self.cache[0] == self.max(size)
    }
    fn is_min(&self, size: usize) -> bool {
        self.cache[0] == self.min(size)
    }
}

#[test]
fn roll_once_test() {
    use crate::init_log;
    use crate::local_now;
    use time::Duration;
    use time::OffsetDateTime;
    use time::{Date, Time};

    let log_name = local_now()
        .format(
            &time::format_description::parse(
                "hurribot_roll_[year]-[month]-[day]T[hour]:[minute]:[second]",
            )
            .unwrap(),
        )
        .unwrap();
    let _logger_guard = init_log(&log_name);
    let mut chart = crate::backtest::candle_chart::CandleChart::read_from_csv(
        "./data/ETHUSDT",
        Duration::minutes(1),
    );
    chart.candles.retain(|c| {
        c.close_time
            > OffsetDateTime::new_utc(
                Date::from_calendar_date(2021, time::Month::March, 26).unwrap(),
                Time::from_hms(0, 0, 0).unwrap(),
            )
    });
    let mut strategy = RollOnceStrategy::new(true, 100., RollConfig::default());
    for c in chart.candles.iter() {
        strategy.update(c);
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

#[test]
fn roll_bull_finder() {
    use crate::init_log;
    use crate::local_now;
    use time::Duration;

    let log_name = local_now()
        .format(
            &time::format_description::parse(
                "hurribot_finder_[year]-[month]-[day]T[hour]:[minute]:[second]",
            )
            .unwrap(),
        )
        .unwrap();
    let _logger_guard = init_log(&log_name);
    let chart = crate::backtest::candle_chart::CandleChart::read_from_csv(
        "./data/PEOPLEUSDT",
        Duration::minutes(1),
    );
    let mut max = CandleData::default();
    let mut entry = CandleData::default();
    let mut start_new = true;
    let mut good_set = Vec::new();
    let mut max_draw: f64 = 0.;
    let mut max_draw_before_max: f64 = 0.;
    let mut max_draw_static = Vec::new();
    let last = chart.candles.last().unwrap().clone();
    for c in chart.candles {
        if start_new {
            entry = c.clone();
            max = c.clone();
            max_draw = 0.;
            max_draw_before_max = 0.;
            start_new = false;
            max_draw_static = vec![];
            continue;
        }
        if c.low < entry.low {
            entry = c.clone();
            max = c.clone();
            max_draw = 0.;
            max_draw_before_max = 0.;
            max_draw_static = vec![];
            continue;
        }
        max = if max.high < c.high {
            if max_draw > max_draw_before_max {
                max_draw_before_max = max_draw;
                max_draw_static.push(((max.high / entry.low - 1.), max_draw));
            }
            c.clone()
        } else {
            max
        };
        max_draw = max_draw.max(1. - (c.low / max.high));
        let increase = max.high / entry.low - 1.;
        if max_draw > (increase * 0.5 + 0.1).min(0.5) {
            if increase > 0.5 {
                good_set.push((
                    entry.clone(),
                    c.clone(),
                    max.clone(),
                    max_draw_static.clone(),
                ));
            }
            start_new = true;
        }
    }
    if !start_new && max.high / entry.low > 1.5 {
        good_set.push((entry.clone(), last, max.clone(), max_draw_static.clone()));
    }
    for (entry, end, max, max_draw_static) in good_set.iter() {
        let mut static_str = String::new();
        for s in max_draw_static {
            static_str.push_str(&format!(
                "increase: {:8.2}%\t\tmax_draw: {:8.2}%\n",
                s.0 * 100.,
                s.1 * 100.
            ));
        }
        info!(
            "\nentry: {:#?}, \nend: {:#?}, \nmax: {:#?}, \nreturn rate: {}, \nmax_draw_static: \n{}",
            entry,
            end,
            max,
            end.close / entry.low,
            static_str
        );
    }
    info!("good set count: {}", good_set.len());
    //create and open file result.csv, write max_raw_static to it

    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new().append(true).open("result.csv").unwrap();
    for (_, _, _, max_draw_static) in good_set.iter() {
        let mut static_str = String::new();
        for s in max_draw_static {
            static_str.push_str(&format!("{:.4}, {:.4}\n", s.0 * 100., s.1 * 100.));
        }
        file.write_all(static_str.as_bytes()).unwrap();
    }
}

#[test]
fn scratch() {
    let _price = 1.;
    let steps = 1000;
    let x = 2.0f64.powf(1. / steps as f64);
    println!("x: {}", x.powf(1000.));
}
