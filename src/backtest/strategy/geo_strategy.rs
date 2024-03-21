use std::sync::{Arc, Mutex};

use time::{Duration, OffsetDateTime};
use tracing::warn;

use crate::backtest::{
    candle_chart::CandleData,
    contract::{Contract, HANDLING_FEE_RATE_MAKER},
};

use super::Strategy;

// 等比定时开仓策略
#[derive(Debug, Clone)]
pub struct GeoStrategy {
    /// 多空
    is_bull: bool,
    /// 杠杆
    leverage: f64,
    /// 开仓间隔
    interval: Duration,
    /// 每次开仓占总资金比例
    ratio: f64,
    /// 若总资金不足则补充到此值
    supply: f64,
    /// 止损比例
    stop_loss_ratio: f64,
    /// 超过间隔后按该比例止盈
    take_profit_ratio: f64,
    /// 当前持仓
    position: Option<Contract>,
    /// 当前资金
    capital: f64,
    /// 后备资金
    stake: f64,
    /// 总成本（补充资金总值）
    pub cost: f64,
    /// 开单次数
    pub open_count: i64,
    /// 上次开单时间
    last_time: OffsetDateTime,
    /// 总资金
    total_capital: Arc<Mutex<f64>>,
}

impl GeoStrategy {
    /// 理论上其他参数固定的情况下，ratio * leverage / (1 + leverage * handling_fee_rate) 相等时，收益率和风险一致
    pub fn new(
        is_bull: bool,
        leverage: f64,
        ratio: f64,
        interval: Duration,
        supply: f64,
        stop_loss_ratio: f64,
        take_profit_ratio: f64,
        total_capital: Arc<Mutex<f64>>,
    ) -> Self {
        if take_profit_ratio < HANDLING_FEE_RATE_MAKER * 2. {
            warn!(
                "take profit ratio is too low, can't take profit unless it is greater than {}",
                1. + HANDLING_FEE_RATE_MAKER * 2.
            );
        }
        Self {
            is_bull,
            leverage,
            interval,
            ratio,
            supply,
            stop_loss_ratio,
            take_profit_ratio,
            position: None,
            capital: 0.,
            stake: 0.,
            cost: 0.,
            open_count: 0,
            last_time: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            total_capital,
        }
    }
}

impl Strategy for GeoStrategy {
    fn update(&mut self, candle: &CandleData) {
        if let Some(contract) = self.position.take() {
            if let Some(r) = contract.liquidate(if self.is_bull {
                candle.low
            } else {
                candle.high
            }) {
                // 止损或强制平仓
                self.capital += r;
            } else if contract.open_time + self.interval <= candle.close_time
                && ((self.is_bull
                    && candle.close > contract.entry_price * (1. + self.take_profit_ratio))
                    || (!self.is_bull
                        && candle.close < contract.entry_price * (1. - self.take_profit_ratio)))
            {
                // 超过间隔后按比例止盈，否则继续持有该仓位
                self.capital += contract.close(candle.close);
            } else {
                self.position = Some(contract);
            }
        }
        if self.position.is_some() || self.last_time + self.interval > candle.close_time {
            // 只有空仓且超过间隔后才开仓
            return;
        }
        if self.capital < self.supply {
            let cost = self.supply - self.capital;
            if self.stake < cost {
                let supplement = cost - self.stake;
                let mut total_capital = self.total_capital.lock().unwrap();
                if *total_capital < supplement {
                    panic!(
                        "total capital = {} is not enough, need {}",
                        *total_capital, cost
                    );
                }
                *total_capital -= supplement;
                self.stake = cost;
                self.cost += supplement;
            }
            self.stake -= cost;
            self.capital = self.supply;
        }
        let stop_loss = if self.is_bull {
            Some((1. - self.stop_loss_ratio) * candle.close)
        } else {
            Some((1. + self.stop_loss_ratio) * candle.close)
        };
        self.position = Some(Contract::open(
            self.is_bull,
            candle.close,
            self.capital * self.ratio,
            self.leverage,
            candle.close_time,
            stop_loss,
        ));
        self.capital -= self.capital * self.ratio;
        self.last_time = candle.close_time;
        self.open_count += 1;
    }
    fn close(&mut self, price: f64) -> f64 {
        if let Some(offer) = self.position.take() {
            self.capital += offer.close(price);
        }
        self.value()
    }
    fn value(&self) -> f64 {
        if let Some(contract) = &self.position {
            contract.margin + self.capital + self.stake
        } else {
            self.capital + self.stake
        }
    }
}
