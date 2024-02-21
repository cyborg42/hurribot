use binance::config::Config;

#[test]
fn binance_test() {
    use binance::futures::websockets::*;
    use std::sync::atomic::AtomicBool;

    let keep_running = AtomicBool::new(true); // Used to control the event loop
    let mut future_web_socket = FuturesWebSockets::new(|event: FuturesWebsocketEvent| {
        println!("Received: {:?}", event);
        
        Ok(())
    });
    let subscribes = vec!["!markPrice@arr".to_string()];
    
    future_web_socket.connect_multiple_streams(&FuturesMarket::USDM, &subscribes).unwrap();
    if let Err(e) = future_web_socket.event_loop(&keep_running) {
        match e {
            err => {
                println!("Error: {:?}", err);
            }
        }
    }
    future_web_socket.disconnect().unwrap();
}
