use std::collections::HashMap;

use anyhow::{anyhow, bail};
use binance::futures::{
    account::{OrderRequest, TimeInForce},
    model::{Bracket, TransactionOrError},
};
use dashmap::DashMap;
use tracing::error;

use crate::{
    binance_futures::{BinanceKeys, Clients},
    stdout_logger,
};

use super::{Market, MarketOrderRequest, MarketOrderReturn};

#[derive(Debug)]
pub struct BinanceSymbolStatus {
    min_qty: f64,
    min_qty_step: f64,
    tick_size: f64,
    min_notional: f64,
    brackets: Vec<Bracket>,
}

impl Default for BinanceSymbolStatus {
    fn default() -> Self {
        Self {
            min_qty: 1.,
            min_qty_step: 1.,
            tick_size: 0.1,
            min_notional: 1.,
            brackets: Vec::new(),
        }
    }
}

impl BinanceSymbolStatus {
    pub fn update_market_info(&mut self, info: binance::futures::model::Symbol) {
        for filter in info.filters {
            match filter {
                binance::model::Filters::LotSize {
                    min_qty, step_size, ..
                } => {
                    self.min_qty = min_qty.parse().unwrap();
                    self.min_qty_step = step_size.parse().unwrap();
                }
                binance::model::Filters::PriceFilter { tick_size, .. } => {
                    self.tick_size = tick_size.parse().unwrap();
                }
                binance::model::Filters::MinNotional {
                    notional: Some(n), ..
                } => {
                    self.min_notional = n.parse().unwrap();
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
pub struct BinanceMarket {
    statuses: DashMap<String, BinanceSymbolStatus>,
    leverage: u8,
    clients: Clients,
}

impl BinanceMarket {
    pub fn new(binance_keys: BinanceKeys, leverage: u8) -> anyhow::Result<Self> {
        let binance_clients = Clients::new(binance_keys);
        binance_clients
            .account
            .change_position_mode(false)
            .inspect_err(|e| {
                error!("change position mode failed: {:?}", e.0);
            })
            .ok();
        let ex_info = binance_clients
            .general
            .exchange_info()
            .map_err(|e| anyhow!("get ex info failed: {:?}", e.0))?;
        let account_info = binance_clients
            .account
            .account_information()
            .map_err(|e| anyhow!("get account info failed: {:?}", e.0))?;
        let leverage_brackets = binance_clients
            .account
            .leverage_brackets(None)
            .map_err(|e| anyhow!("get leverage bracket failed: {:?}", e.0))?;
        let mut ret = Self {
            statuses: DashMap::new(),
            clients: binance_clients,
            leverage,
        };
        let mut map = HashMap::new();
        for s in ex_info.symbols {
            map.entry(s.symbol.clone()).or_insert((s, None, None));
        }
        for a in account_info.positions {
            map.entry(a.symbol.clone()).and_modify(|e| e.1 = Some(a));
        }
        for b in leverage_brackets {
            map.entry(b.symbol.clone())
                .and_modify(|e| e.2 = Some(b.brackets));
        }

        for (symbol, (symbol_info, position, brackets)) in map {
            let mut status = BinanceSymbolStatus::default();
            status.update_market_info(symbol_info);

            if let Some(p) = position {
                if !p.isolated {
                    ret.clients
                        .account
                        .change_margin_type(&symbol, true)
                        .map_err(|e| {
                            anyhow!("Symbol {} change margin type failed: {:?}", symbol, e.0)
                        })?;
                }
                let l: u8 = p.leverage.parse()?;
                if l != ret.leverage {
                    ret.clients
                        .account
                        .change_initial_leverage(&symbol, ret.leverage)
                        .map_err(|e| {
                            anyhow!("Symbol {} change leverage failed: {:?}", symbol, e.0)
                        })?;
                }
            } else {
                error!("Symbol {} position not found", symbol);
                continue;
            }
            if let Some(b) = brackets {
                status.brackets = b;
            } else {
                error!("Symbol {} brackets not found", symbol);
                continue;
            }
            ret.statuses.insert(symbol, status);
        }
        Ok(ret)
    }
    pub fn update_symbol_status(&self, symbol: &str, is_force: bool) -> anyhow::Result<()> {
        if self.statuses.contains_key(symbol) && !is_force {
            return Ok(());
        }
        let mut status = BinanceSymbolStatus::default();
        let symbol_info = self
            .clients
            .general
            .get_symbol_info(symbol)
            .map_err(|e| anyhow!("get symbol info failed: {:?}", e.0))?;
        status.update_market_info(symbol_info);
        let position = self
            .clients
            .account
            .account_information()
            .map_err(|e| anyhow!("get account info failed: {:?}", e.0))?
            .positions
            .into_iter()
            .find(|p| p.symbol == symbol)
            .ok_or(anyhow!("position not found"))?;
        if !position.isolated {
            self.clients
                .account
                .change_margin_type(symbol, true)
                .map_err(|e| anyhow!("Symbol {} change margin type failed: {:?}", symbol, e.0))?;
        }
        let l: u8 = position.leverage.parse()?;
        if l != self.leverage {
            self.clients
                .account
                .change_initial_leverage(symbol, self.leverage)
                .map_err(|e| anyhow!("Symbol {} change leverage failed: {:?}", symbol, e.0))?;
        }
        let brackets = self
            .clients
            .account
            .leverage_brackets(Some(symbol.to_string()))
            .map_err(|e| anyhow!("get leverage bracket failed: {:?}", e.0))?
            .pop()
            .ok_or(anyhow!("brackets not found"))?
            .brackets;
        status.brackets = brackets;

        self.statuses.insert(symbol.to_string(), status);
        Ok(())
    }
}

impl Market for BinanceMarket {
    fn clear_orders(&self, symbol: &str) -> anyhow::Result<()> {
        self.clients
            .account
            .cancel_all_open_orders(symbol)
            .map_err(|e| anyhow!("cancel all open orders failed: {:?}", e.0))?;
        Ok(())
    }
    fn close_position(&self, symbol: &str) -> anyhow::Result<()> {
        self.clear_orders(symbol)?;
        let position = self
            .clients
            .account
            .position_information(symbol.to_string())
            .map_err(|e| anyhow!("get position failed: {:?}", e.0))?
            .pop()
            .ok_or(anyhow!("position not found"))?;
        if position.position_amount == 0. {
            return Ok(());
        }
        if position.position_amount > 0. {
            self.clients
                .account
                .market_sell(symbol, position.position_amount)
                .map_err(|e| anyhow!("market sell failed: {:?}", e.0))?;
        } else {
            self.clients
                .account
                .market_buy(symbol, -position.position_amount)
                .map_err(|e| anyhow!("market buy failed: {:?}", e.0))?;
        }
        Ok(())
    }
    fn order(&self, request: MarketOrderRequest) -> anyhow::Result<MarketOrderReturn> {
        let symbol = request.symbol.clone();
        let position_risk = self
            .clients
            .account
            .position_information(symbol.clone())
            .map_err(|e| anyhow!("get position risk failed: {:?}", e.0))?
            .pop()
            .ok_or(anyhow!("position risk not found"))?;
        if position_risk.position_amount != 0. {
            bail!("position not empty");
        }
        self.clear_orders(&symbol)?;
        self.update_symbol_status(&symbol, false)?;
        let status = self
            .statuses
            .get(&symbol)
            .ok_or(anyhow!("status not found"))?;
        let price = self
            .clients
            .market
            .get_price(&symbol)
            .map_err(|e| anyhow!("get price failed: {:?}", e.0))?
            .price;
        let qty = truncate(request.value / price, status.min_qty_step);
        let executed_value = qty * price;
        if qty < status.min_qty {
            bail!("qty too small");
        }
        if executed_value < status.min_notional {
            bail!("min notional not satisfied");
        }
        let low_price = truncate(price * request.low_limit, status.tick_size);
        let high_price = truncate(price * request.high_limit, status.tick_size);
        let bracket = status
            .brackets
            .iter()
            .find(|b| executed_value >= b.notional_floor && executed_value <= b.notional_cap)
            .ok_or(anyhow!("bracket not found"))?;
        if self.leverage > bracket.initial_leverage {
            bail!("leverage/value too high");
        }
        let mut orders = Vec::new();
        if request.is_buy {
            orders.push(OrderRequest::market_buy(&symbol, qty));
            let mut order = OrderRequest::limit_sell(&symbol, qty, high_price, TimeInForce::GTC);
            order.reduce_only = Some(true);
            orders.push(order);
            orders.push(OrderRequest::stop_market_close_sell(&symbol, low_price));
        } else {
            orders.push(OrderRequest::market_sell(&symbol, qty));
            let mut order = OrderRequest::limit_buy(&symbol, qty, low_price, TimeInForce::GTC);
            order.reduce_only = Some(true);
            orders.push(order);
            orders.push(OrderRequest::stop_market_close_buy(&symbol, high_price));
        }
        let transactions = self
            .clients
            .account
            .custom_batch_orders(orders)
            .map_err(|e| anyhow!("batch order failed: {:?}", e.0))?;
        let maintenance_margin = executed_value * bracket.maint_margin_ratio - bracket.cum;
        let default_margin = executed_value / self.leverage as f64;
        let target_margin = if request.is_buy {
            qty * (price - low_price) + maintenance_margin
        } else {
            qty * (high_price - price) + maintenance_margin
        };
        let additional_margin = target_margin - default_margin;
        if additional_margin > 0. {
            let m = additional_margin + 0.01;
            self.clients
                .account
                .change_position_margin(&symbol, m, true)
                .map_err(|e| anyhow!("add position margin failed: {:?}", e.0))?;
        }
        let order_id = match transactions.first().unwrap() {
            TransactionOrError::Transaction(t) => t.order_id,
            TransactionOrError::Error(e) => bail!("order failed: {:?}", e),
        };
        Ok(MarketOrderReturn {
            order_id,
            qty,
            value: executed_value,
        })
    }
}

fn truncate(value: f64, step: f64) -> f64 {
    (value / step).trunc() * step
}

#[test]
fn market_test() {
    stdout_logger();
    let binance_keys = BinanceKeys::value_parse("./config/binance_keys.toml").unwrap();
    let market = BinanceMarket::new(binance_keys, 20);
    println!("{:?}", market);
}
