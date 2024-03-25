use crate::signal::SignalType;

use super::{Algorithm, SymbolPriceInfo};

#[derive(Debug, Clone)]
pub struct RollAlgrithm {
    price: SymbolPriceInfo,
}
impl RollAlgrithm {
    pub fn new() -> Self {
        Self {
            price: SymbolPriceInfo {
                price: 0.0,
                update_time: 0,
                funding_rate: 0.0,
            },
        }
    }
}
impl Algorithm for RollAlgrithm {
    fn init(&mut self, price_info: &SymbolPriceInfo) {
        self.price = price_info.clone();
    }
    fn update(&mut self, symbol_status: &SymbolPriceInfo) -> Option<f64> {
        self.price = symbol_status.clone();
        Some(self.price.price)
    }
    fn get_signal_type() -> SignalType {
        SignalType::Roll
    }
}
