use std::{fs::File, path::Path};

use tracing::{error, info, warn};

#[derive(Debug)]
pub struct CandleChart {
    /// k线间隔（秒）
    interval: i64,
    pub candles: Vec<CandleData>,
}

impl CandleChart {
    pub fn new(interval: i64) -> Self {
        Self {
            interval,
            candles: vec![],
        }
    }
    /// open_time               K线图开盘时间（unix格式）
    /// open                    开盘价
    /// high                    最高价
    /// low                     最低价
    /// close                   收盘价
    /// volume                  成交量
    /// close_time              K线图收盘时间（unix格式）
    /// quote_volume            报价币成交量
    /// count                   成单数
    /// taker_buy_volume        在此期间吃单方买入的基础币数量
    /// taker_buy_quote_volume  在此期间吃单方买入的报价币数量
    /// ignore                  忽略
    pub fn read_from_csv(path: &str, interval: i64) -> Self {
        info!("read from csv: {}", path);
        let path = Path::new(path);
        let mut chart = Self::new(interval);
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();

                if path.is_file() {
                    let file = File::open(path).unwrap();
                    let mut csv = csv::Reader::from_reader(file);

                    for d in csv.records() {
                        let d = d.unwrap();
                        let time = d.get(0).unwrap().parse::<i64>().unwrap() / 1000;
                        let open = d.get(1).unwrap().parse::<f64>().unwrap();
                        let high = d.get(2).unwrap().parse::<f64>().unwrap();
                        let low = d.get(3).unwrap().parse::<f64>().unwrap();
                        let close = d.get(4).unwrap().parse::<f64>().unwrap();
                        let volume = d.get(5).unwrap().parse::<f64>().unwrap();
                        chart.candles.push(CandleData {
                            open,
                            high,
                            low,
                            close,
                            volume,
                            time,
                        });
                    }
                }
            }
        } else if path.is_file() {
            let file = File::open(path).unwrap();
            let mut csv = csv::Reader::from_reader(file);

            for d in csv.records() {
                let d = d.unwrap();
                let time = d.get(0).unwrap().parse::<i64>().unwrap() / 1000;
                let open = d.get(1).unwrap().parse::<f64>().unwrap();
                let high = d.get(2).unwrap().parse::<f64>().unwrap();
                let low = d.get(3).unwrap().parse::<f64>().unwrap();
                let close = d.get(4).unwrap().parse::<f64>().unwrap();
                let volume = d.get(5).unwrap().parse::<f64>().unwrap();
                chart.candles.push(CandleData {
                    open,
                    high,
                    low,
                    close,
                    volume,
                    time,
                });
            }
        }

        chart.candles.sort();
        chart
    }
}

#[derive(Debug)]
pub struct CandleData {
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub time: i64,
}

impl Ord for CandleData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for CandleData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for CandleData {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for CandleData {}
