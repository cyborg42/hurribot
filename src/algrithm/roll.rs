use crate::market::SymbolUpdateInfo;

use super::Algrithm;

#[derive(Default, Debug)]
pub struct Roll {
    status: SymbolUpdateInfo,
}
impl Algrithm for Roll {
    fn update(&mut self, symbol_status: SymbolUpdateInfo)  {
        self.status = symbol_status;
    }
    fn get_value(&self) -> f64 {
        0.
    }
}
