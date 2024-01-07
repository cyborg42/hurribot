use std::sync::{Arc, Mutex};

use crate::{
    candle_chart::CandleData,
    contract::{Contract, HANDLING_FEE_RATE_MAKER, HANDLING_FEE_RATE_TAKER},
};
use tracing::{error, info, warn};

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
