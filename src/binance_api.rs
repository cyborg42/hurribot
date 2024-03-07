use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::JoinHandle,
};

use binance::futures::websockets::{FuturesMarket, FuturesWebSockets, FuturesWebsocketEvent};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct BinanceConfig {
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

struct FutureWsConnection {
    market: FuturesMarket,
    subscribes: Vec<String>,
}
impl FutureWsConnection {
    pub fn new(market: FuturesMarket, subscribes: Vec<String>) -> Self {
        Self { market, subscribes }
    }
    pub fn run<F>(&self, handler: F, keep_running: Arc<AtomicBool>) -> JoinHandle<()>
    where
        F: FnMut(FuturesWebsocketEvent) -> binance::errors::Result<()> + Send + 'static,
    {
        let sub = self.subscribes.clone();

        let market = self.market;
        std::thread::Builder::new()
            .name("FutureWsConnection".to_string())
            .spawn(move || {
                let mut future_ws = FuturesWebSockets::new(handler);

                loop {
                    if let Err(e) = future_ws.connect_multiple_streams(&market, &sub) {
                        println!("Error: {:?}", e);
                        break;
                    }
                    if let Err(e) = future_ws.event_loop(&keep_running) {
                        match e.0 {
                            binance::errors::ErrorKind::Msg(err) => {
                                println!("Error: {:?}", err);
                                if err.starts_with("Disconnected") {
                                    continue;
                                }
                            }
                            err => {
                                println!("Error: {:?}", err);
                            }
                        }
                    }
                    break;
                }
                future_ws.disconnect().ok();
            })
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binance::api::Binance;
    #[test]
    fn future_ws_test() {
        use binance::futures::websockets::*;
        use std::sync::atomic::AtomicBool;
        //let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

        let keep_running = AtomicBool::new(true); // Used to control the event loop
        let mut future_ws = FuturesWebSockets::new(|event: FuturesWebsocketEvent| {
            println!("Received: {:?}", event);
            Ok(())
        });
        let subscribes = vec!["!markPrice@arr".to_string()];

        loop {
            if let Err(e) = future_ws.connect_multiple_streams(&FuturesMarket::USDM, &subscribes) {
                println!("Error: {:?}", e);
                break;
            }
            if let Err(e) = future_ws.event_loop(&keep_running) {
                match e.0 {
                    binance::errors::ErrorKind::Msg(err) => {
                        println!("Error: {:?}", err);
                        if err.starts_with("Disconnected") {
                            continue;
                        }
                    }
                    err => {
                        println!("Error: {:?}", err);
                    }
                }
            }
            break;
        }
        future_ws.disconnect().unwrap();
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
        use binance::futures::userstream::FuturesUserStream;
        use std::sync::atomic::AtomicBool;
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();
        let user_stream = FuturesUserStream::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        let keep_running = AtomicBool::new(true); // Used to control the event loop
        let mut listen_key = "".to_string();
        if let Ok(answer) = user_stream.start() {
            listen_key = answer.listen_key;
            let mut future_ws = binance::futures::websockets::FuturesWebSockets::new(|event| {
                println!("Received: {:?}", event);
                Ok(())
            });
            future_ws
                .connect(
                    &binance::futures::websockets::FuturesMarket::USDM,
                    &listen_key,
                )
                .unwrap();
            println!("{:?}", future_ws.event_loop(&keep_running));
        }
        user_stream.keep_alive(&listen_key).unwrap();
    }
}
