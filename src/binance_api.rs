use std::{
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
    time::Duration,
};

use binance::{
    api::Binance,
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
                    if err.starts_with("Disconnected") {
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Clone, Debug)]
pub enum FutureWsConnection {
    MarketData(Vec<String>),
    UserData(BinanceConfig),
}
impl FutureWsConnection {
    pub fn run<F>(&self, market: FuturesMarket, handler: F, running: Arc<AtomicBool>)
    where
        F: FnMut(FuturesWebsocketEvent) -> binance::errors::Result<()> + Send + 'static,
    {
        let c = self.clone();
        std::thread::spawn(move || {
            match c {
                Self::MarketData(sub) => {
                    let mut future_ws = FuturesWebSockets::new(handler);
                    loop {
                        if let Err(e) = future_ws.connect_multiple_streams(&market, &sub) {
                            error!("Init connection error, exiting...: {:?}", e);
                            break;
                        }
                        if !future_ws.event_loop_reconnect(&running) {
                            break;
                        }
                    }
                }
                Self::UserData(config) => {
                    let user_stream = FuturesUserStream::new(
                        Some(config.api_key.clone()),
                        Some(config.secret_key.clone()),
                    );
                    let mut future_ws = FuturesWebSockets::new(handler);
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
                        if let Err(e) = future_ws.connect(&market, listen_key) {
                            error!("Init connection error, exiting...: {:?}", e);
                            break;
                        }
                        if !future_ws.event_loop_reconnect(&running) {
                            break;
                        }
                    }
                }
            };
            running.store(false, Relaxed);
        });
    }
}

pub struct BinanceClients {
    pub config: BinanceConfig,
    pub general: binance::futures::general::FuturesGeneral,
    pub market: binance::futures::market::FuturesMarket,
    pub account: binance::futures::account::FuturesAccount,
}
impl BinanceClients {
    pub fn new(config: BinanceConfig) -> Self {
        let general = binance::futures::general::FuturesGeneral::new(
            Some(config.api_key.clone()),
            Some(config.secret_key.clone()),
        );
        let market = binance::futures::market::FuturesMarket::new(
            Some(config.api_key.clone()),
            Some(config.secret_key.clone()),
        );
        let account = binance::futures::account::FuturesAccount::new(
            Some(config.api_key.clone()),
            Some(config.secret_key.clone()),
        );
        Self {
            config,
            general,
            market,
            account,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use crate::stdout_logger;

    use super::*;
    use binance::api::Binance;

    #[test]
    fn future_ws_test() {
        use binance::futures::websockets::*;
        use std::sync::atomic::AtomicBool;
        stdout_logger();
        let running = Arc::new(AtomicBool::new(true)); // Used to control the event loop
        let handler = move |event: FuturesWebsocketEvent| {
            println!("Received: {:?}", event);
            Ok(())
        };

        let subscribes = vec!["!markPrice@arr".to_string()];

        let conn = FutureWsConnection::MarketData(subscribes);
        conn.run(FuturesMarket::USDM, handler, running.clone());

        sleep(Duration::from_secs(60));
    }
    #[test]
    fn future_rest_test() {
        use binance::futures;
        stdout_logger();
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

        let general = futures::general::FuturesGeneral::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        let market = futures::market::FuturesMarket::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        let account = futures::account::FuturesAccount::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        market.get_all_prices().unwrap();
        println!("{:#?}", account.account_information().unwrap());
        println!("{:#?}", general.get_symbol_info("BTCUSDT").unwrap());
        //println!("{:#?}", general.exchange_info());
    }
    #[test]
    fn future_account_test() {
        use binance::futures::account::FuturesAccount;
        stdout_logger();
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

        let account = FuturesAccount::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        println!("{:#?}", account.account_information());
    }
    #[test]
    fn future_account_ws_test() {
        use std::sync::atomic::AtomicBool;
        stdout_logger();
        let config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
        let keep_running = Arc::new(AtomicBool::new(true)); // Used to control the event loop
        let conn = FutureWsConnection::UserData(config);
        let handler = |event: FuturesWebsocketEvent| {
            println!("Received: {:?}", event);
            Ok(())
        };
        conn.run(FuturesMarket::USDM, handler, keep_running);
        sleep(Duration::from_secs(60));
    }
}
