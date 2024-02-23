use binance::config::Config;
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
#[cfg(test)]
mod tests {
    use super::*;
    use binance::{
        api::{Binance, Futures},
        errors::BinanceContentError,
    };
    #[test]
    fn future_ws_test() {
        use binance::futures::websockets::*;
        use std::sync::atomic::AtomicBool;
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

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
        let market = futures::market::FuturesMarket::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );
        let account = futures::account::FuturesAccount::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );

        println!("{:#?}", general.exchange_info());
    }
    #[test]
    fn future_account_test() {
        use binance::futures::account::FuturesAccount;
        let binance_config = BinanceConfig::value_parse("./config/binance_config.toml").unwrap();

        let account = FuturesAccount::new(
            Some(binance_config.api_key.clone()),
            Some(binance_config.secret_key.clone()),
        );

        println!("{:?}", account.position_information("SOLUSDT"));
    }
}
