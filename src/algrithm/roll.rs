use crate::{market::SymbolPriceInfo, signal::SignalType};

use super::Algrithm;

#[derive(Debug)]
pub struct RollAlgrithm {}
impl Algrithm for RollAlgrithm {
    fn update(&mut self, symbol_status: &SymbolPriceInfo) -> Option<(SignalType, f64)> {
        None
    }
}
