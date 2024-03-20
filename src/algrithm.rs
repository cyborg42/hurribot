use crate::market::SymbolUpdateInfo;

pub mod roll;

pub trait Algrithm: std::fmt::Debug + Default {
    fn update(&mut self, symbol_status: SymbolUpdateInfo);
    fn get_value(&self) -> f64;
}



