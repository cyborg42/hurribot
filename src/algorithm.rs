use std::collections::HashMap;

use crossbeam::channel::Sender;

use crate::{
    market::SymbolPriceInfo,
    signal::{Signal, SignalType},
};

pub mod roll;

pub trait Algorithm: Clone {
    fn init(&mut self, price_info: &SymbolPriceInfo);
    fn update(&mut self, price_info: &SymbolPriceInfo) -> Option<f64>;
    fn get_signal_type() -> SignalType;
}

pub struct AlgorithmsManager<A> {
    pub template: A,
    pub algorithms: HashMap<String, A>,
    pub signal_tx: Sender<Signal>,
}

impl<A> AlgorithmsManager<A> {
    pub fn new(template: A, signal_tx: Sender<Signal>) -> Self {
        Self {
            template,
            algorithms: HashMap::new(),
            signal_tx,
        }
    }
}

macro_rules! impl_algorithms_manager {
    ($($T:ident $idx:tt),+) => {
        impl<$($T),+> AlgorithmsManager<($($T),+,)>
        where
            $( $T: Algorithm, )+
        {
            pub fn update(&mut self, symbol: String, price_info: SymbolPriceInfo) {
                self.algorithms
                    .entry(symbol.clone())
                    .and_modify(|a| {
                        $(
                            if let Some(value) = a.$idx.update(&price_info) {
                                self.signal_tx
                                    .send(Signal {
                                        signal_type: <$T as Algorithm>::get_signal_type(),
                                        symbol: symbol.clone(),
                                        value,
                                    })
                                    .unwrap();
                            }
                        )+
                    })
                    .or_insert_with(|| {
                        ($({
                            let mut t = self.template.$idx.clone();
                            t.init(&price_info);
                            t
                        },)+)
                    });
            }
        }
    };
}

impl_algorithms_manager!(A 0);
impl_algorithms_manager!(A 0, B 1);
impl_algorithms_manager!(A 0, B 1, C 2);
impl_algorithms_manager!(A 0, B 1, C 2, D 3);
