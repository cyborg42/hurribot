use crossbeam::channel::{unbounded, Receiver};


#[derive(Debug, Clone, Default)]
pub struct SymbolPrice {
    pub symbol: String,
    pub mark_price: f64,
    pub price_index: f64,
    pub time: u64,
    pub funding_rate: f64,
}

