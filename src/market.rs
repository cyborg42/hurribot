use std::collections::HashMap;

use crate::{
    algrithm::Algrithm,
    binance_futures::{BinanceConfig, Clients},
};
use anyhow::anyhow;
use binance::futures;
use tracing::error;

#[derive(Debug, Clone)]
pub struct SymbolPriceInfo {
    pub symbol: String,
    pub price: f64,
    pub update_time: u64,
    pub funding_rate: f64,
}

#[derive(Debug)]
pub struct SymbolStatus {
    leverage: u8,
    isolated: bool,
    min_qty: f64,
    min_qty_step: f64,
    tick_size: f64,
    min_notional: f64,
}

impl SymbolStatus {
    pub fn new() -> Self {
        Self {
            leverage: 0,
            isolated: false,
            min_qty: 0.,
            min_qty_step: 0.,
            tick_size: 0.,
            min_notional: 0.,
        }
    }
    pub fn update_market_info(&mut self, info: binance::futures::model::Symbol) {
        for filter in info.filters {
            match filter {
                binance::model::Filters::LotSize {
                    min_qty, step_size, ..
                } => {
                    self.min_qty = min_qty.parse().unwrap_or_default();
                    self.min_qty_step = step_size.parse().unwrap_or_default();
                }
                binance::model::Filters::PriceFilter { tick_size, .. } => {
                    self.tick_size = tick_size.parse().unwrap_or_default();
                }
                binance::model::Filters::MinNotional {
                    notional: Some(n), ..
                } => {
                    self.min_notional = n.parse().unwrap_or_default();
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
pub struct MarketStatus {
    symbols: HashMap<String, SymbolStatus>,

    total_wallet_balance: f64,
    available_balance: f64,

    clients: Clients,
    // orders: Vec<Order>,
    // positions: Vec<Position>,
}

impl MarketStatus {
    pub fn new(binance_config: BinanceConfig) -> anyhow::Result<Self> {
        let binance_clients = Clients::new(binance_config);

        let ex_info = binance_clients
            .general
            .exchange_info()
            .map_err(|e| anyhow!("get ex info failed: {}", e))?;
        let account_info = binance_clients
            .account
            .account_information()
            .map_err(|e| anyhow!("get account info failed: {}", e))?;

        let mut ret = Self {
            symbols: HashMap::new(),
            total_wallet_balance: account_info.total_wallet_balance,
            available_balance: account_info.available_balance,
            clients: binance_clients,
        };
        let mut map = HashMap::new();
        for s in ex_info.symbols {
            map.entry(s.symbol.clone()).or_insert((s, None));
        }
        for a in account_info.positions {
            map.entry(a.symbol.clone()).and_modify(|e| e.1 = Some(a));
        }

        for (symbol_name, (symbol, position)) in map {
            let mut status = SymbolStatus::new();
            status.update_market_info(symbol);

            if let Some(p) = position {
                if !p.isolated {
                    if let Err(e) = ret.clients.account.change_margin_type(&symbol_name, true) {
                        error!("Symbol {} change margin type failed: {}", symbol_name, e);
                        continue;
                    }
                }
                status.leverage = p.leverage.parse().unwrap_or_default();
                status.isolated = true;
            } else {
                error!("Symbol {} position not found", symbol_name);
                continue;
            }

            ret.symbols.insert(symbol_name, status);
        }
        Ok(ret)
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }
}

#[test]
fn market_test() {
    let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
    let market = MarketStatus::new(binance_config);
    println!("{:#?}", market);
}
