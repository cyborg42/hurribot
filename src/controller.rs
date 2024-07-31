use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread::{sleep, JoinHandle},
    time::Duration,
};

use anyhow::{anyhow, Ok};
use binance::{futures::model::OrderUpdate, model::AccountUpdateDataEvent};
use crossbeam::channel::{Receiver, Select, Sender};
use dashmap::DashMap;
use parking_lot::Mutex;
use rayon::{prelude::*, Scope};
use tracing::error;

use crate::{
    algorithm::{ SymbolPrice},
    market::{Market, MarketOrderRequest},
    strategy::{Strategy, StrategyOrderReturn},
};

#[derive(Debug)]
struct Controller<M> {
    market: M,
    strategies: Vec<Box<dyn Strategy>>,
    // prices: Arc<DashMap<String, SymbolPrice>>,
    total_balance: Mutex<f64>,
    cross_balance: Mutex<f64>,
    open_orders: DashMap<u64, Order>,
    positions: DashMap<String, Position>,
    update_time: AtomicU64,
}

impl<M: Market> Controller<M> {
    fn run(
        self,
        signal_rx: Receiver<SymbolPrice>,
        account_rx: Receiver<AccountInfo>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            rayon::ThreadPoolBuilder::new()
                .num_threads(4)
                .use_current_thread()
                .build()
                .unwrap()
                .scope(|s| loop {
                    crossbeam::channel::select! {
                        recv(signal_rx) -> signal => {
                            s.spawn(|_| self.input_signal(signal.unwrap()));
                        }
                        recv(account_rx) -> account_info => {
                            s.spawn(|_| self.update_account(account_info.unwrap()));
                        }
                    }
                })
        })
    }

    fn input_signal(&self, signal: SymbolPrice) {
        for strategy in self.strategies.iter() {
            if let Some(order_request) = strategy.update(&signal) {
                let market_order_request = MarketOrderRequest::new(
                    order_request.symbol,
                    true,
                    order_request.position * *self.cross_balance.lock(),
                    order_request.stop_loss,
                    order_request.take_profit,
                )
                .unwrap();

                self.market.order(market_order_request);
                // send order request to exchange
            }
        }
    }
    fn update_account(&self, account_info: AccountInfo) {
        match account_info {
            AccountInfo::OrderTrade { time, order } => {
                // update open orders and positions
            }
            AccountInfo::AccountUpdate { time, data } => {
                self.update_time.store(time, Ordering::Relaxed);
                for b in data.balances {
                    if b.asset == "USDT" {
                        *self.total_balance.lock() = b.wallet_balance.parse().unwrap();
                        *self.cross_balance.lock() = b.cross_wallet_balance.parse().unwrap();
                    }
                }
                for p in data.positions {
                    let mut position = self.positions.entry(p.symbol.clone()).or_default();
                    position.entry_price = p.entry_price.parse().unwrap();
                    position.position_amount = p.position_amount.parse().unwrap();
                    position.isolated_wallet = p.isolated_wallet.parse().unwrap();
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct Position {
    entry_price: f64,
    position_amount: f64,
    isolated_wallet: f64,
}
#[derive(Debug)]
pub struct Order {}

pub enum AccountInfo {
    OrderTrade {
        time: u64,
        order: Box<OrderUpdate>,
    },
    AccountUpdate {
        time: u64,
        data: AccountUpdateDataEvent,
    },
}
