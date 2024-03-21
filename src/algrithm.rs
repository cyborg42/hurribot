use std::collections::HashMap;

use crossbeam::channel::Sender;

use crate::{
    market::SymbolPriceInfo,
    signal::{Signal, SignalType},
};

pub mod roll;

pub trait Algrithm {
    fn update(&mut self, price_info: &SymbolPriceInfo) -> Option<(SignalType, f64)>;
}

pub struct Algrithms {
    pub algrithms: Vec<Box<dyn Algrithm>>,
    pub signal_tx: Sender<Signal>,
}
impl Algrithms {
    pub fn new(algrithms: Vec<Box<dyn Algrithm>>, signal_tx: Sender<Signal>) -> Self {
        Self {
            algrithms,
            signal_tx,
        }
    }
    pub fn update(&mut self, price_info: SymbolPriceInfo) {
        for algrithm in self.algrithms.iter_mut() {
            if let Some(value) = algrithm.update(&price_info) {
                self.signal_tx
                    .send(Signal {
                        signal_type: value.0,
                        symbol: price_info.symbol.clone(),
                        value: value.1,
                    })
                    .unwrap();
            }
        }
    }
}
