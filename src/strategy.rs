use std::fmt::Debug;


use crate::{algorithm::{ SymbolPrice}, controller::Order};

pub mod roll;

pub trait Strategy: Debug + Send + Sync {
    fn notify(&self, order_return: StrategyOrderReturn);
    fn update(&self, price: &SymbolPrice) -> Option<StrategyOrderRequest>;
}

pub struct StrategyOrderReturn {
    pub request_id: u64,
    pub result: anyhow::Result<Order>,
}

pub struct StrategyOrderRequest {
    pub request_id: u64,
    pub symbol: String,
    pub position: f64,
    /// 0 < stop_loss < 1
    pub stop_loss: f64,
    /// take_profit > 1
    pub take_profit: f64,
}
