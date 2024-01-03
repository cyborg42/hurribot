use std::sync::{Mutex, Arc};

use crate::{contract::{Contract, HANDLING_FEE_RATE_MAKER, HANDLING_FEE_RATE_TAKER}, candle_chart::CandleData};
use tracing::{error, warn, info};



// 等比定时开仓策略
#[derive(Debug, Clone)]
pub struct GeoStrategy {
    /// 杠杆
    leverage: f64,
    /// 开仓间隔
    interval: i64,
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
    last_time: i64,
    /// 总资金
    total_capital: Arc<Mutex<f64>>,
}

impl GeoStrategy {
    pub fn new(
        leverage: f64,
        ratio: f64,
        interval: i64,
        supply: f64,
        stop_loss_ratio: f64,
        take_profit_ratio: f64,
        total_capital: Arc<Mutex<f64>>,
    ) -> Self {
        if take_profit_ratio < 1. + HANDLING_FEE_RATE_MAKER * 2. {
            warn!(
                "take profit ratio is too low, can't take profit unless it is greater than {}",
                1. + HANDLING_FEE_RATE_MAKER * 2.
            );
        }
        Self {
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
            last_time: -10000000000,
            total_capital,
        }
    }
    pub fn update(&mut self, candle: &CandleData) {
        if let Some(contract) = self.position.take() {
            if let Some(r) = contract.liquidate(candle.low) {
                // 止损或强制平仓
                self.capital += r;
            } else if contract.open_time + self.interval <= candle.time
                && candle.close > contract.entry_price * self.take_profit_ratio
            {
                // 超过间隔后按比例止盈，否则继续持有该仓位
                self.capital += contract.close(candle.close);
            } else {
                self.position = Some(contract);
            }
        }
        if self.position.is_some() || self.last_time + self.interval > candle.time {
            // 只有空仓且超过间隔后才开仓
            return;
        }
        if self.capital < self.supply {
            let cost = self.supply - self.capital;
            if self.stake < cost {
                let supplement = cost - self.stake;
                let mut total_capital = self.total_capital.lock().unwrap();
                if *total_capital < supplement {
                    error!(
                        "total capital = {} is not enough, need {}",
                        *total_capital, cost
                    );
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
        self.position = Some(Contract::open(
            candle.close,
            self.capital * self.ratio,
            self.leverage,
            candle.time,
            Some(self.stop_loss_ratio * candle.close),
        ));
        self.capital -= self.capital * self.ratio;
        self.last_time = candle.time;
        self.open_count += 1;
    }
    pub fn close(&mut self, price: f64) {
        if let Some(offer) = self.position.take() {
            self.capital += offer.close(price);
        }
    }
    pub fn value(&self) -> f64 {
        if let Some(offer) = &self.position {
            offer.margin + self.capital + self.stake
        } else {
            self.capital + self.stake
        }
    }
}
