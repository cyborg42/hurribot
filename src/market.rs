use anyhow::{anyhow, Ok};
use crossbeam::channel::{Receiver, Sender};
use rayon::{
    iter::{ParallelBridge, ParallelIterator},
    result,
};

pub mod binance_market;

pub trait Market: std::fmt::Debug + Send + Sync + 'static {
    fn run(self, request_rx: Receiver<MarketRequest>, result_tx: Sender<MarketResult>)
    where
        Self: Sized,
    {
        std::thread::spawn(move || {
            request_rx.iter().par_bridge().for_each(|req| {
                match req {
                    MarketRequest::ClearOrders(symbol) => {
                        let _ = self.clear_orders(&symbol);
                    }
                    MarketRequest::ClosePosition(symbol) => {
                        let _ = self.close_position(&symbol);
                    }
                    MarketRequest::Order(request) => {
                        let _ = self.order(request);
                    }
                }
                result_tx.send(MarketResult {}).unwrap();
            });
        });
    }
    fn clear_orders(&self, symbol: &str) -> anyhow::Result<()>;
    fn close_position(&self, symbol: &str) -> anyhow::Result<()>;
    fn order(&self, request: MarketOrderRequest) -> anyhow::Result<MarketOrderReturn>;
}

pub struct MarketResult {}

pub enum MarketRequest {
    ClearOrders(String),
    ClosePosition(String),
    Order(MarketOrderRequest),
}
pub struct MarketOrderRequest {
    symbol: String,
    is_buy: bool,
    value: f64,
    low_limit: f64,
    high_limit: f64,
}

impl MarketOrderRequest {
    pub fn new(
        symbol: String,
        is_buy: bool,
        value: f64,
        low_limit: f64,
        high_limit: f64,
    ) -> anyhow::Result<Self> {
        if value <= 0. {
            return Err(anyhow!("value must be positive"));
        }
        if low_limit <= 0. || low_limit >= 1. {
            return Err(anyhow!("low_limit must be in (0, 1)"));
        }
        if high_limit <= 1. {
            return Err(anyhow!("high_limit must be greater than 1"));
        }
        Ok(Self {
            symbol,
            is_buy,
            value,
            low_limit,
            high_limit,
        })
    }
}
pub struct MarketOrderReturn {
    pub order_id: u64,
    pub qty: f64,
    pub value: f64,
}
