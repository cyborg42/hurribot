use std::collections::HashMap;

use crossbeam::channel::{unbounded, Receiver};

pub mod roll;

#[derive(Debug, Clone, Default)]
pub struct SymbolPrice {
    pub mark_price: f64,
    pub price_index: f64,
    pub time: u64,
    pub funding_rate: f64,
}

pub trait Algorithm: Clone {
    fn init(&mut self, price_info: &SymbolPrice);
    fn update(&mut self, price_info: &SymbolPrice) -> Option<SignalData>;
}

pub struct AlgorithmsManager<A> {
    template: A,
    algorithms: HashMap<String, A>,
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
                price_rx: Receiver<(String, SymbolPrice)>
            ) -> (Receiver<Signal>, std::thread::JoinHandle<()>) {
                let (s_tx, s_rx) = unbounded();
                (
                    s_rx,
                    std::thread::spawn(move || {
                        for p in price_rx.iter() {
                            for s in self.update(p.0, p.1){
                                s_tx.send(s).unwrap();
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
            pub fn update(&mut self, symbol: String, price_info: SymbolPrice) -> Vec<Signal> {
                let mut v = vec![];
                self.algorithms
                    .entry(symbol.clone())
                    .and_modify(|a| {
                        $(
                            if let Some(data) = a.$idx.update(&price_info) {
                                v.push(Signal {
                                        symbol: symbol.clone(),
                                        time: price_info.time,
                                        data,
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

#[derive(Debug, Clone)]
pub struct Signal {
    pub data: SignalData,
    pub symbol: String,
    pub time: u64,
}
#[derive(Debug, Clone)]
pub enum SignalData {
    Roll(f64),
    Grid,
}
