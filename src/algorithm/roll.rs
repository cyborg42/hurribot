use super::{Algorithm, SignalData, SymbolPrice};

#[derive(Debug, Clone)]
pub struct RollAlgo {
    price: SymbolPrice,
}
impl RollAlgo {
    pub fn new() -> Self {
        Self {
            price: SymbolPrice::default(),
        }
    }
}
impl Algorithm for RollAlgo {
    fn init(&mut self, price_info: &SymbolPrice) {
        self.price = price_info.clone();
    }
    fn update(&mut self, symbol_status: &SymbolPrice) -> Option<SignalData> {
        self.price = symbol_status.clone();
        None
    }
}
