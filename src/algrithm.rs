use crate::market::SymbolPriceInfo;

pub mod roll;

pub trait Algrithm: std::fmt::Debug {
    fn new(price_info: SymbolPriceInfo) -> Self;
    fn update(&mut self, price_info: SymbolPriceInfo);
    fn get_value(&self) -> f64;
}
