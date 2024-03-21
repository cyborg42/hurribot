use std::{
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
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
use serde::Deserialize;
use tracing::{error, info, warn};

trait FuturesWebSocketsExt {
    fn event_loop_reconnect(&mut self, running: &AtomicBool) -> bool;
}

impl<'a> FuturesWebSocketsExt for FuturesWebSockets<'a> {
    fn event_loop_reconnect(&mut self, running: &AtomicBool) -> bool {
        if let Err(e) = self.event_loop(running) {
            match e.0 {
                binance::errors::ErrorKind::Msg(err) => {
                    if err.contains("Disconnected") || err.contains("UserDataStreamExpiredEvent") {
                        warn!("Disconnected from binance, reconnecting...: {}", err);
                        self.disconnect().ok();
                        return true;
                    }
                    error!("Event loop error, exiting...: {:?}", err);
                }
                binance::errors::ErrorKind::Tungstenite(err) => {
                    warn!("Connection error, reconnecting...: {}", err);
                    self.disconnect().ok();
                    return true;
                }
                err => {
                    error!("Unexpected error, exiting...: {}", err);
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
    pub fn run<F>(&self, mut handler: F, running: Arc<AtomicBool>)
    where
        F: FnMut(FuturesWebsocketEvent) -> binance::errors::Result<()> + Send + 'static,
    {
        let c = self.clone();
        std::thread::spawn(move || {
            match c {
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
        });
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
    use std::thread::sleep;

    use crate::stdout_logger;

    use super::*;

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
        conn.run(handler, running.clone());
        sleep(Duration::from_secs(60));
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
        let running = Arc::new(AtomicBool::new(true)); // Used to control the event loop
        let handler = |event: FuturesWebsocketEvent| {
            println!("Received: {:?}", event);
            Ok(())
        };
        let conn = FuturesWsConnection::UserData(config);
        conn.run(handler, running);
        sleep(Duration::from_secs(60));
    }
}
