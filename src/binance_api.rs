use std::{
    sync::{atomic::AtomicBool, Arc},
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
                    error!("Event loop error: {:?}", err);
                    if err.starts_with("Disconnected") {
                        info!("Try to reconnect to binance...");
                        self.disconnect().ok();
                        return true;
                    }
                }
                binance::errors::ErrorKind::Tungstenite(err) => {
                    error!("Connection error: {:?}", err);
                    info!("Try to reconnect to binance...");
                    self.disconnect().ok();
                    return true;
                }
                err => {
                    error!("Unexpected error, exit...: {:?}", err);
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
    pub fn run<F>(&self, market: FuturesMarket, handler: F, keep_running: Arc<AtomicBool>)
    where
        F: FnMut(FuturesWebsocketEvent) -> binance::errors::Result<()> + Send + 'static,
    {
        match self.clone() {
            Self::MarketData(sub) => std::thread::spawn(move || {
                let mut future_ws = FuturesWebSockets::new(handler);
                loop {
                    if let Err(e) = future_ws.connect_multiple_streams(&market, &sub) {
                        error!("Init connection error, exit...: {:?}", e);
                        break;
                    }
                    if !future_ws.event_loop_reconnect(&keep_running) {
                        break;
                    }
                }
            }),
            Self::UserData(config) => std::thread::spawn(move || {
                let user_stream = FuturesUserStream::new(
                    Some(config.api_key.clone()),
                    Some(config.secret_key.clone()),
                );
                let mut future_ws = FuturesWebSockets::new(handler);
                let mut listen_key_last = String::new();
                loop {
                    let answer = match user_stream.start() {
                        Ok(answer) => answer,
                        Err(e) => {
                            error!("Request for listen key failed: {:?}", e);
                            break;
                        }
                    };
                    if answer.listen_key != listen_key_last {
                        let (u_c, l_c) = (user_stream.clone(), answer.listen_key.clone());
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
                        listen_key_last = answer.listen_key.clone();
                    }

                    let listen_key: &'static String =
                        unsafe { std::mem::transmute(&answer.listen_key) };
                    if let Err(e) = future_ws.connect(&market, listen_key) {
                        error!("Init connection error, exit...: {:?}", e);
                        break;
                    }
                    if !future_ws.event_loop_reconnect(&keep_running) {
                        info!("Exit...");
                        break;
                    }
                }
            }),
        };
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;
    use binance::api::Binance;
    #[test]
    fn future_ws_test() {
        use binance::futures::websockets::*;
        use std::sync::atomic::AtomicBool;

        let keep_running = Arc::new(AtomicBool::new(true)); // Used to control the event loop
        let handler = |event: FuturesWebsocketEvent| {
            println!("Received: {:?}", event);
            Ok(())
        };
        let subscribes = vec!["!markPrice@arr".to_string()];

        let conn = FutureWsConnection::MarketData(subscribes);
        conn.run(FuturesMarket::USDM, handler, keep_running);
        sleep(Duration::from_secs(60));
    }
    #[test]
    fn future_rest_test() {
        use binance::futures;
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

        let general = futures::general::FuturesGeneral::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        let _market = futures::market::FuturesMarket::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        let _account = futures::account::FuturesAccount::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );

        println!("{:#?}", general.get_symbol_info("BTCUSDT"));
    }
    #[test]
    fn future_account_test() {
        use binance::futures::account::FuturesAccount;
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

        let account = FuturesAccount::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        println!(
            "{:#?}",
            account.change_position_margin("SOLUSDT", 0.1, false)
        );
    }
    #[test]
    fn future_account_ws_test() {
        use std::sync::atomic::AtomicBool;
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
