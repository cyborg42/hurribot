use crate::{market::SymbolPriceInfo, signal::SignalType};

use super::Algorithm;

#[derive(Debug, Clone)]
pub struct RollAlgrithm {}
impl Algorithm for RollAlgrithm {
    fn init(&mut self, _price_info: &SymbolPriceInfo) {}
    fn update(&mut self, _symbol_status: &SymbolPriceInfo) -> Option<f64> {
        None
    }
    fn get_signal_type() -> SignalType {
        SignalType::Roll
    }
}
