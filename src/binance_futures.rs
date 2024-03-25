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
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::algorithm::SymbolPriceInfo;

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
                    error!("Unexpected error, exiting...: {}", e);
                }
            }
        }
        self.disconnect().ok();
        false
    }
}

#[derive(Clone, Deserialize)]
pub struct BinanceConfig {
    pub api_key: String,
    pub secret_key: String,
}
impl BinanceConfig {
    pub fn value_parse(path: &str) -> Result<Self, String> {
        let c = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let val: Self = toml::from_str(&c).map_err(|e| e.to_string())?;
        Ok(val)
    }
}
impl core::fmt::Debug for BinanceConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BinanceConfig").finish()
    }
}

#[derive(Clone, Debug)]
pub enum FuturesWsConnection {
    MarketData(Vec<String>),
    UserData(BinanceConfig),
}
impl FuturesWsConnection {
    pub fn run_price_info() -> (Receiver<(String, SymbolPriceInfo)>, JoinHandle<()>) {
        let (price_tx, price_rx) = crossbeam::channel::unbounded();
        let running = Arc::new(AtomicBool::new(true));
        let handler = move |event: FuturesWebsocketEvent| {
            match event {
                FuturesWebsocketEvent::MarkPriceAll(v) => {
                    v.into_iter().for_each(|p| {
                        let s = SymbolPriceInfo {
                            price: p.mark_price.parse().unwrap_or_default(),
                            update_time: p.event_time,
                            funding_rate: p.funding_rate.parse().unwrap_or_default(),
                        };
                        price_tx.send((p.symbol, s)).unwrap();
                    });
                }
                _ => {}
            }
            Ok(())
        };
        let subscribes = vec!["!markPrice@arr".to_string()];
        let conn = FuturesWsConnection::MarketData(subscribes);
        let h = conn.run(handler, running.clone());
        (price_rx, h)
    }
    pub fn run_account_info(
        binance_config: BinanceConfig,
    ) -> (Receiver<FuturesWebsocketEvent>, JoinHandle<()>) {
        let (account_tx, account_rx) = crossbeam::channel::unbounded();
        let running = Arc::new(AtomicBool::new(true));
        let handler = move |event: FuturesWebsocketEvent| {
            account_tx.send(event).unwrap();
            Ok(())
        };
        let conn = FuturesWsConnection::UserData(binance_config);
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
                        let user_key = match user_stream.start() {
                            Ok(u) => u,
                            Err(e) => {
                                error!("Request for listen key failed, exiting...: {:?}", e);
                                break;
                            }
                        };
                        if user_key.listen_key != listen_key_last {
                            let (u_c, l_c) = (user_stream.clone(), user_key.listen_key.clone());
                            std::thread::spawn(move || loop {
                                match u_c.keep_alive(&l_c) {
                                    Ok(_) => {
                                        info!("Listen key {} extended.", l_c);
                                        std::thread::sleep(Duration::from_secs(50 * 60))
                                    }
                                    Err(e) => {
                                        warn!("Listen key {} dropped: {:?}", l_c, e);
                                        break;
                                    }
                                }
                            });
                            listen_key_last = user_key.listen_key.clone();
                        }
                        let listen_key: &'static str =
                            unsafe { std::mem::transmute(user_key.listen_key.as_str()) };
                        if let Err(e) = futures_ws.connect(&FuturesMarket::USDM, listen_key) {
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
    pub config: BinanceConfig,
    pub general: binance::futures::general::FuturesGeneral,
    pub market: binance::futures::market::FuturesMarket,
    pub account: binance::futures::account::FuturesAccount,
}
impl Clients {
    pub fn new(config: BinanceConfig) -> Self {
        let c = Config {
            recv_window: 10000,
            ..Default::default()
        };
        let general = binance::futures::general::FuturesGeneral::new_with_config(
            Some(config.api_key.clone()),
            Some(config.secret_key.clone()),
            &c,
        );
        let market = binance::futures::market::FuturesMarket::new_with_config(
            Some(config.api_key.clone()),
            Some(config.secret_key.clone()),
            &c,
        );
        let account = binance::futures::account::FuturesAccount::new_with_config(
            Some(config.api_key.clone()),
            Some(config.secret_key.clone()),
            &c,
        );
        Self {
            config,
            general,
            market,
            account,
        }
    }
}

impl core::fmt::Debug for Clients {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Clients").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdout_logger;
    #[test]
    fn ws() {
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
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
        let clients = Clients::new(binance_config);
        clients.market.get_all_prices().unwrap();
        println!("{:#?}", clients.account.account_information());
        //println!("{:#?}", clients.general.get_symbol_info("BTCUSDT").unwrap());
        //println!("{:#?}", clients.general.exchange_info());
    }
    #[test]
    fn account() {
        stdout_logger();
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
        let clients = Clients::new(binance_config);
        println!("{:#?}", clients.account.account_information());
    }
    #[test]
    fn account_ws() {
        use std::sync::atomic::AtomicBool;
        stdout_logger();
        let config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
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
    }
}
