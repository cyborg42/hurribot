use std::{
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use binance::{
    api::Binance,
    config::Config,
    futures::{
        userstream::FuturesUserStream,
        websockets::{FuturesMarket, FuturesWebSockets, FuturesWebsocketEvent},
    },
};
use crossbeam::channel::Receiver;
use dashmap::DashMap;
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::{algorithm::SymbolPrice, controller::AccountInfo};

trait FuturesWebSocketsExt {
    fn event_loop_reconnect(&mut self, running: &AtomicBool) -> bool;
}

impl<'a> FuturesWebSocketsExt for FuturesWebSockets<'a> {
    fn event_loop_reconnect(&mut self, running: &AtomicBool) -> bool {
        if let Err(e) = self.event_loop(running) {
            match e.0 {
                binance::errors::ErrorKind::Msg(e) => {
                    if e.contains("Disconnected") || e.contains("UserDataStreamExpiredEvent") {
                        warn!("Disconnected from binance, reconnecting...: {}", e);
                        self.disconnect().ok();
                        return true;
                    }
                    error!("Event loop error, exiting...: {:?}", e);
                }
                binance::errors::ErrorKind::Tungstenite(e) => {
                    warn!("Connection error, reconnecting...: {}", e);
                    self.disconnect().ok();
                    return true;
                }
                e => {
                    error!("Unexpected error, exiting...: {:?}", e);
                }
            }
        }
        self.disconnect().ok();
        false
    }
}

#[derive(Clone, Deserialize)]
pub struct BinanceKeys {
    pub api_key: String,
    pub secret_key: String,
}
impl BinanceKeys {
    pub fn value_parse(path: &str) -> anyhow::Result<Self> {
        let c = std::fs::read_to_string(path)?;
        let val: Self = toml::from_str(&c)?;
        Ok(val)
    }
}
opaque_debug::implement!(BinanceKeys);

#[derive(Clone, Debug)]
pub enum FuturesWsConnection {
    MarketData(Vec<String>),
    UserData(BinanceKeys),
}
impl FuturesWsConnection {
    pub fn run_price_info() -> (
        Receiver<SymbolPrice>,
        Arc<DashMap<String, SymbolPrice>>,
        JoinHandle<()>,
    ) {
        let prices = Arc::new(DashMap::new());
        let prices_c = prices.clone();
        let (price_tx, price_rx) = crossbeam::channel::unbounded();
        let running = Arc::new(AtomicBool::new(true));
        let handler = move |event: FuturesWebsocketEvent| {
            if let FuturesWebsocketEvent::MarkPriceAll(v) = event {
                v.into_iter().for_each(|p| {
                    if p.symbol.contains('_') || p.symbol.contains("USDC") {
                        return;
                    }
                    let mark_price: f64 = p.mark_price.parse().unwrap_or_default();
                    let price_index: f64 = p
                        .index_price
                        .map(|s| s.parse().unwrap_or_default())
                        .unwrap_or(mark_price);
                    let s = SymbolPrice {
                        symbol: p.symbol.clone(),
                        mark_price,
                        price_index,
                        time: p.event_time,
                        funding_rate: p.funding_rate.parse().unwrap_or_default(),
                    };
                    prices_c.insert(p.symbol.clone(), s.clone());
                    price_tx.send(s).unwrap();
                });
            }
            Ok(())
        };
        let subscribes = vec!["!markPrice@arr@1s".to_string()];
        let conn = FuturesWsConnection::MarketData(subscribes);
        let h = conn.run(handler, running.clone());
        (price_rx, prices, h)
    }
    pub fn run_account_info(binance_keys: BinanceKeys) -> (Receiver<AccountInfo>, JoinHandle<()>) {
        let (account_tx, account_rx) = crossbeam::channel::unbounded();
        let running = Arc::new(AtomicBool::new(true));
        let handler = move |event: FuturesWebsocketEvent| {
            info!("Account Stream Received: {:?}", event);
            match event {
                FuturesWebsocketEvent::OrderTrade(e) => {
                    account_tx
                        .send(AccountInfo::OrderTrade {
                            time: e.event_time,
                            order: Box::new(e.order),
                        })
                        .unwrap();
                }
                FuturesWebsocketEvent::AccountUpdate(e) => {
                    account_tx
                        .send(AccountInfo::AccountUpdate {
                            time: e.event_time,
                            data: e.data,
                        })
                        .unwrap();
                }
                _ => {}
            }
            Ok(())
        };
        let conn = FuturesWsConnection::UserData(binance_keys);
        let h = conn.run(handler, running.clone());
        (account_rx, h)
    }
    pub fn run<F>(self, mut handler: F, running: Arc<AtomicBool>) -> JoinHandle<()>
    where
        F: FnMut(FuturesWebsocketEvent) -> binance::errors::Result<()> + Send + 'static,
    {
        std::thread::spawn(move || {
            match self {
                Self::MarketData(sub) => {
                    let mut futures_ws = FuturesWebSockets::new(handler);
                    loop {
                        if let Err(e) =
                            futures_ws.connect_multiple_streams(&FuturesMarket::USDM, &sub)
                        {
                            error!("Init connection error, exiting...: {:?}", e);
                            break;
                        }
                        if !futures_ws.event_loop_reconnect(&running) {
                            break;
                        }
                    }
                }
                Self::UserData(config) => {
                    let user_stream = FuturesUserStream::new(
                        Some(config.api_key.clone()),
                        Some(config.secret_key.clone()),
                    );
                    let handler = |e: FuturesWebsocketEvent| {
                        if let FuturesWebsocketEvent::UserDataStreamExpiredEvent(_) = e {
                            error_chain::bail!("UserDataStreamExpiredEvent");
                        }
                        handler(e)
                    };
                    let mut futures_ws = FuturesWebSockets::new(handler);
                    let mut listen_key_last = String::new();
                    loop {
                        let listen_key = match user_stream.start() {
                            Ok(u) => u.listen_key,
                            Err(e) => {
                                error!("Request for listen key failed, exiting...: {:?}", e);
                                break;
                            }
                        };
                        if listen_key != listen_key_last {
                            let (u_c, l_c) = (user_stream.clone(), listen_key.clone());
                            std::thread::spawn(move || loop {
                                match u_c.keep_alive(&l_c) {
                                    Ok(_) => {
                                        info!("Listen key {} extended.", l_c);
                                        std::thread::sleep(Duration::from_secs(50 * 60))
                                    }
                                    Err(e) => {
                                        warn!("Listen key {} dropped: {:?}", l_c, e.0);
                                        break;
                                    }
                                }
                            });
                            listen_key_last = listen_key.clone();
                        }
                        if let Err(e) = futures_ws.connect(&FuturesMarket::USDM, &listen_key) {
                            error!("Init connection error, exiting...: {:?}", e);
                            break;
                        }
                        if !futures_ws.event_loop_reconnect(&running) {
                            break;
                        }
                    }
                }
            };
            running.store(false, Relaxed);
        })
    }
}

pub struct Clients {
    pub general: binance::futures::general::FuturesGeneral,
    pub market: binance::futures::market::FuturesMarket,
    pub account: binance::futures::account::FuturesAccount,
}
impl Clients {
    pub fn new(keys: BinanceKeys) -> Self {
        let config = Config {
            recv_window: 10000,
            ..Default::default()
        };
        let general = binance::futures::general::FuturesGeneral::new_with_config(
            Some(keys.api_key.clone()),
            Some(keys.secret_key.clone()),
            &config,
        );
        let market = binance::futures::market::FuturesMarket::new_with_config(
            Some(keys.api_key.clone()),
            Some(keys.secret_key.clone()),
            &config,
        );
        let account = binance::futures::account::FuturesAccount::new_with_config(
            Some(keys.api_key.clone()),
            Some(keys.secret_key.clone()),
            &config,
        );
        Self {
            general,
            market,
            account,
        }
    }
}
opaque_debug::implement!(Clients);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::stdout_logger;
    #[test]
    fn ws() {
        // TODO: 测试指定价格穿透频率
        use binance::futures::websockets::*;
        use std::sync::atomic::AtomicBool;
        stdout_logger();
        let running = Arc::new(AtomicBool::new(true)); // Used to control the event loop
        let handler = move |event: FuturesWebsocketEvent| {
            println!("Received: {:?}", event);
            Ok(())
        };
        let subscribes = vec!["!markPrice@arr".to_string()];
        let conn = FuturesWsConnection::MarketData(subscribes);
        conn.run(handler, running.clone()).join().unwrap();
    }
    #[test]
    fn rest() {
        stdout_logger();
        let binance_keys = BinanceKeys::value_parse("./config/binance_keys.toml").unwrap();
        let clients = Clients::new(binance_keys);
        //clients.market.get_all_prices().unwrap();
        match clients.account.change_position_mode(false) {
            Ok(_) => {}
            Err(e) => {
                println!("{:?}", e.0);
            }
        }
        //println!("{:#?}", clients.general.get_symbol_info("BTCUSDT").unwrap());
        //println!("{:#?}", clients.general.exchange_info());
        // Transaction {
        //     client_order_id: "A5oOsPNLxpZFEH4Y7z1Qam",
        //     cum_qty: 0.0,
        //     cum_quote: 0.0,
        //     executed_qty: 0.0,
        //     order_id: 46319290669,
        //     avg_price: 0.0,
        //     orig_qty: 1.0,
        //     reduce_only: false,
        //     side: "BUY",
        //     position_side: "BOTH",
        //     status: "NEW",
        //     stop_price: 0.0,
        //     close_position: false,
        //     symbol: "SOLUSDT",
        //     time_in_force: "GTC",
        //     type_name: "MARKET",
        //     orig_type: "MARKET",
        //     activate_price: None,
        //     price_rate: None,
        //     update_time: 1712042629058,
        //     working_type: "CONTRACT_PRICE",
        //     price_protect: false,
        // },
    }
    #[test]
    fn account() {
        stdout_logger();
        let binance_keys = BinanceKeys::value_parse("./config/binance_keys.toml").unwrap();
        let clients = Clients::new(binance_keys);
        println!("{:#?}", clients.account.account_information());
    }
    #[test]
    fn account_ws() {
        use std::sync::atomic::AtomicBool;
        stdout_logger();
        let config = BinanceKeys::value_parse("./config/binance_keys.toml").unwrap();
        let running = Arc::new(AtomicBool::new(true));
        let handler = |event: FuturesWebsocketEvent| {
            println!("Received: {:?}", event);
            Ok(())
        };
        let conn = FuturesWsConnection::UserData(config);
        conn.run(handler, running).join().unwrap();
        // Received: OrderTrade(OrderTradeEvent { event_type: "ORDER_TRADE_UPDATE", event_time: 1711310062035, transaction_time: 1711310062035, order: OrderUpdate { symbol: "SOLUSDT", new_client_order_id: "ios_mO5PYJzaUuK8SVCt4eQL", side: "BUY", order_type: "MARKET", time_in_force: "GTC", qty: "1", price: "0", average_price: "0", stop_price: "0", execution_type: "NEW", order_status: "NEW", order_id: 45348952648, qty_last_filled_trade: "0", accumulated_qty_filled_trades: "0", price_last_filled_trade: "0", asset_commisioned: None, commission: Some("0"), trade_order_time: 1711310062035, trade_id: 0, bids_notional: "0", ask_notional: "0", is_buyer_maker: false, is_reduce_only: false, stop_price_working_type: "CONTRACT_PRICE", original_order_type: "MARKET", position_side: "BOTH", close_all: Some(false), activation_price: None, callback_rate: None, pp_ignore: false, si_ignore: 0, ss_ignore: 0, realized_profit: "0" } })
        // Received: AccountUpdate(AccountUpdateEvent { event_type: "ACCOUNT_UPDATE", event_time: 1711310062035, data: AccountUpdateDataEvent { reason: "ORDER", balances: [EventBalance { asset: "USDT", wallet_balance: "1091.96321610", cross_wallet_balance: "1047.81330743", balance_change: "0" }], positions: [EventPosition { symbol: "SOLUSDT", position_amount: "1", entry_price: "176.614", accumulated_realized: "-1698.49199986", unrealized_pnl: "0.00359133", margin_type: "isolated", isolated_wallet: "44.14990867", position_side: "BOTH" }] } })
        // Received: OrderTrade(OrderTradeEvent { event_type: "ORDER_TRADE_UPDATE", event_time: 1711310062035, transaction_time: 1711310062035, order: OrderUpdate { symbol: "SOLUSDT", new_client_order_id: "ios_mO5PYJzaUuK8SVCt4eQL", side: "BUY", order_type: "MARKET", time_in_force: "GTC", qty: "1", price: "0", average_price: "176.6140", stop_price: "0", execution_type: "TRADE", order_status: "FILLED", order_id: 45348952648, qty_last_filled_trade: "1", accumulated_qty_filled_trades: "1", price_last_filled_trade: "176.6140", asset_commisioned: None, commission: Some("0.08830700"), trade_order_time: 1711310062035, trade_id: 1443769856, bids_notional: "0", ask_notional: "0", is_buyer_maker: false, is_reduce_only: false, stop_price_working_type: "CONTRACT_PRICE", original_order_type: "MARKET", position_side: "BOTH", close_all: Some(false), activation_price: None, callback_rate: None, pp_ignore: false, si_ignore: 0, ss_ignore: 0, realized_profit: "0" } })
        // Received: AccountUpdate(AccountUpdateEvent { event_type: "ACCOUNT_UPDATE", event_time: 1711986263150, data: AccountUpdateDataEvent { reason: "WITHDRAW", balances: [EventBalance { asset: "USDT", wallet_balance: "1079.36036130", cross_wallet_balance: "941.93470639", balance_change: "-10" }], positions: [] } })
        // Received: AccountUpdate(AccountUpdateEvent { event_type: "ACCOUNT_UPDATE", event_time: 1711986271902, data: AccountUpdateDataEvent { reason: "DEPOSIT", balances: [EventBalance { asset: "USDT", wallet_balance: "1089.36036130", cross_wallet_balance: "951.93470639", balance_change: "10" }], positions: [] } })
    }
}
