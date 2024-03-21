use crate::market::SymbolPriceInfo;

use super::Algrithm;

#[derive(Debug)]
pub struct RollAlgrithm {
    status: SymbolPriceInfo,
}
impl Algrithm for RollAlgrithm {
    fn new(symbol_status: SymbolPriceInfo) -> Self {
        Self {
            status: symbol_status,
        }
    }
    fn update(&mut self, symbol_status: SymbolPriceInfo) {
        self.status = symbol_status;
    }
    fn get_value(&self) -> f64 {
        0.
    }
}
