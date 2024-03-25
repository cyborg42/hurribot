use std::collections::HashMap;

use crossbeam::channel::{unbounded, Receiver, Sender};

use crate::signal::{Signal, SignalType};

pub mod roll;

#[derive(Debug, Clone)]
pub struct SymbolPriceInfo {
    pub price: f64,
    pub update_time: u64,
    pub funding_rate: f64,
}

pub trait Algorithm: Clone {
    fn init(&mut self, price_info: &SymbolPriceInfo);
    fn update(&mut self, price_info: &SymbolPriceInfo) -> Option<f64>;
    fn get_signal_type() -> SignalType;
}

pub struct AlgorithmsManager<A> {
    pub template: A,
    pub algorithms: HashMap<String, A>,
}

impl<A> AlgorithmsManager<A> {
    pub fn new(template: A) -> Self {
        Self {
            template,
            algorithms: HashMap::new(),
        }
    }
}

macro_rules! impl_algorithms_manager {
    ($($T:ident $idx:tt),+) => {
        impl<$($T),+> AlgorithmsManager<($($T),+,)>
        where
            $( $T: Algorithm + Send + 'static, )+
        {
            pub fn run(
                mut self,
                price_rx: Receiver<(String, SymbolPriceInfo)>
            ) -> (Receiver<Signal>, std::thread::JoinHandle<()>) {
                let (tx, rx) = unbounded();
                (
                    rx,
                    std::thread::spawn(move || {
                        for p in price_rx.iter() {
                            for s in self.update(p.0, p.1){
                                tx.send(s).unwrap();
                            }
                        }
                    })
                )
            }
        }

        impl<$($T),+> AlgorithmsManager<($($T),+,)>
        where
            $( $T: Algorithm, )+
        {
            pub fn update(&mut self, symbol: String, price_info: SymbolPriceInfo) -> Vec<Signal> {
                let mut v = vec![];
                self.algorithms
                    .entry(symbol.clone())
                    .and_modify(|a| {
                        $(
                            if let Some(value) = a.$idx.update(&price_info) {
                                v.push(Signal {
                                        signal_type: <$T as Algorithm>::get_signal_type(),
                                        symbol: symbol.clone(),
                                        value,
                                });
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
                v
            }
        }
    };
}

impl_algorithms_manager!(A 0);
impl_algorithms_manager!(A 0, B 1);
impl_algorithms_manager!(A 0, B 1, C 2);
impl_algorithms_manager!(A 0, B 1, C 2, D 3);
