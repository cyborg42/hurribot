use std::collections::HashMap;

use crate::{
    algrithm::Algrithm,
    binance_api::{BinanceClients, BinanceConfig},
};
use anyhow::anyhow;
use binance::futures;
use tracing::error;

pub struct MarketStatus<A> {
    symbols: HashMap<String, SymbolStatus<A>>,

    total_wallet_balance: f64,
    available_balance: f64,

    binance_clients: BinanceClients,
    // orders: Vec<Order>,
    // positions: Vec<Position>,
}

impl<A: std::fmt::Debug> std::fmt::Debug for MarketStatus<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketStatus")
            .field("symbols", &self.symbols)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SymbolUpdateInfo {
    pub price: f64,
    pub update_time: u64,
    pub funding_rate: f64,
}

impl<A: Algrithm> MarketStatus<A> {
    pub fn new(binance_config: BinanceConfig) -> anyhow::Result<Self> {
        let binance_clients = BinanceClients::new(binance_config);

        let symbols = binance_clients
            .general
            .exchange_info()
            .map_err(|_| anyhow!("get ex info failed"))?;
        let account_info = binance_clients
            .account
            .account_information()
            .map_err(|_| anyhow!("get account info failed"))?;
        let prices = match binance_clients
            .market
            .get_mark_prices()
            .map_err(|_| anyhow!("get mark price failed"))?
        {
            futures::model::MarkPrices::AllMarkPrices(p) => p,
        };
        let mut ret = Self {
            symbols: HashMap::new(),
            total_wallet_balance: account_info.total_wallet_balance,
            available_balance: account_info.available_balance,
            binance_clients,
        };
        let mut map = HashMap::new();
        for s in symbols.symbols {
            map.entry(s.symbol.clone()).or_insert((Some(s), None, None));
        }
        for a in account_info.positions {
            map.entry(a.symbol.clone()).and_modify(|e| e.1 = Some(a));
        }
        for p in prices {
            map.entry(p.symbol.clone()).and_modify(|e| e.2 = Some(p));
        }

        for (symbol_name, (symbol, position, price)) in map {
            let mut status = SymbolStatus::<A>::default();
            let symbol = match symbol {
                Some(s) => s,
                None => {
                    error!("Symbol {} not found", symbol_name);
                    continue;
                }
            };

            for filter in symbol.filters {
                match filter {
                    binance::model::Filters::LotSize {
                        min_qty, step_size, ..
                    } => {
                        status.min_qty = min_qty.parse().unwrap_or_default();
                        status.min_step_qty = step_size.parse().unwrap_or_default();
                    }
                    binance::model::Filters::PriceFilter { tick_size, .. } => {
                        status.tick_size = tick_size.parse().unwrap_or_default();
                    }
                    binance::model::Filters::MinNotional {
                        notional: Some(n), ..
                    } => {
                        status.min_notional = n.parse().unwrap_or_default();
                    }
                    _ => {}
                }
            }
            if let Some(p) = position {
                status.leverage = p.leverage.parse().unwrap_or_default();
                status.isolated = p.isolated;
                if !status.isolated {
                    if let Err(e) = ret
                        .binance_clients
                        .account
                        .change_margin_type(&symbol.symbol, true)
                    {
                        error!("Symbol {} change margin type failed: {}", symbol.symbol, e);
                    }
                }
            }
            if let Some(p) = price {
                let s = SymbolUpdateInfo {
                    price: p.mark_price,
                    update_time: p.time,
                    funding_rate: p.last_funding_rate,
                };
                status.algrithm.update(s);
            }
            ret.symbols.insert(symbol_name, status);
        }
        Ok(ret)
    }

    pub fn update(&mut self, symbol: String, status: SymbolUpdateInfo) {
        self.symbols
            .entry(symbol)
            .or_insert_with_key(|s| {
                let mut status = SymbolStatus::default();
                if let Ok(symbol_info) = self.binance_clients.general.get_symbol_info(s) {
                    for filter in symbol_info.filters {
                        match filter {
                            binance::model::Filters::LotSize {
                                min_qty, step_size, ..
                            } => {
                                status.min_qty = min_qty.parse().unwrap_or_default();
                                status.min_step_qty = step_size.parse().unwrap_or_default();
                            }
                            binance::model::Filters::PriceFilter { tick_size, .. } => {
                                status.tick_size = tick_size.parse().unwrap_or_default();
                            }
                            binance::model::Filters::MinNotional {
                                notional: Some(n), ..
                            } => {
                                status.min_notional = n.parse().unwrap_or_default();
                            }
                            _ => {}
                        }
                    }
                }

                self.binance_clients
                    .account
                    .change_margin_type(s, true)
                    .ok();
                self.binance_clients
                    .account
                    .change_initial_leverage(s, 20)
                    .ok();
                status.isolated = true;
                status.leverage = 20;
                status
            })
            .algrithm
            .update(status);
    }
    pub fn len(&self) -> usize {
        self.symbols.len()
    }
}

#[derive(Debug, Default)]
pub struct SymbolStatus<A> {
    algrithm: A,
    leverage: u32,
    isolated: bool,
    min_qty: f64,
    min_step_qty: f64,
    tick_size: f64,
    min_notional: f64,
}

#[test]
fn market_test() {
    let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
    let mut market: MarketStatus<crate::algrithm::roll::Roll> =
        MarketStatus::new(binance_config).unwrap();
    market.update(
        "BTCUSDT".to_string(),
        SymbolUpdateInfo {
            price: 10000.0,
            update_time: 100,
            funding_rate: 0.0,
        },
    );

    dbg!(market);
}
