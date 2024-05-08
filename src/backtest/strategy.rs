use super::candle_chart::CandleData;

pub trait Strategy {
    fn update(&mut self, candle: &CandleData);
    #[allow(unused_variables)]
    fn close(&mut self, price: f64) -> f64 {
        self.value()
    }
    fn value(&self) -> f64;
}

pub mod geo_strategy;
pub mod roll_strategy;
