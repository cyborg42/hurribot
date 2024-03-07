use std::{fs::File, path::Path};

use time::{Duration, OffsetDateTime};
use tracing::info;

#[derive(Debug)]
pub struct CandleChart {
    /// k线间隔（秒）
    interval: Duration,
    pub candles: Vec<CandleData>,
}

impl CandleChart {
    pub fn new(interval: Duration) -> Self {
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
    pub fn read_from_csv(path: &str, interval: Duration) -> Self {
        fn read_from_csv_file(path: &Path) -> Vec<CandleData> {
            let file = File::open(path).unwrap();
            let mut csv = csv::Reader::from_reader(file);
            let mut candles = vec![];

            for d in csv.records() {
                let d = d.unwrap();
                let open_nano = d.get(0).unwrap().parse::<i64>().unwrap() as i128 * 1_000_000;
                let close_nano = d.get(6).unwrap().parse::<i64>().unwrap() as i128 * 1_000_000;
                let open_time = OffsetDateTime::from_unix_timestamp_nanos(open_nano).unwrap();
                let close_time = OffsetDateTime::from_unix_timestamp_nanos(close_nano).unwrap();
                let open = d.get(1).unwrap().parse::<f64>().unwrap();
                let high = d.get(2).unwrap().parse::<f64>().unwrap();
                let low = d.get(3).unwrap().parse::<f64>().unwrap();
                let close = d.get(4).unwrap().parse::<f64>().unwrap();
                let volume = d.get(5).unwrap().parse::<f64>().unwrap();
                candles.push(CandleData {
                    open,
                    high,
                    low,
                    close,
                    volume,
                    open_time,
                    close_time,
                });
            }
            candles
        }
        info!("read from csv: {}", path);
        let path = Path::new(path);
        let mut chart = Self::new(interval);
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();

                if path.is_file() {
                    chart.candles.append(&mut read_from_csv_file(&path));
                }
            }
        } else if path.is_file() {
            chart.candles = read_from_csv_file(path);
        } else {
            panic!("invalid path: {}", path.display());
        }

        chart.candles.sort();
        chart
    }
}

#[derive(Debug, Clone)]
pub struct CandleData {
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub open_time: OffsetDateTime,
    pub close_time: OffsetDateTime,
}

impl Default for CandleData {
    fn default() -> Self {
        Self {
            open: 0.,
            close: 0.,
            high: 0.,
            low: 0.,
            volume: 0.,
            open_time: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            close_time: OffsetDateTime::from_unix_timestamp(0).unwrap(),
        }
    }
}

impl Ord for CandleData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.close_time.cmp(&other.close_time)
    }
}

impl PartialOrd for CandleData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for CandleData {
    fn eq(&self, other: &Self) -> bool {
        self.close_time == other.close_time
    }
}

impl Eq for CandleData {}

#[test]
fn candle_test() {
    let close_time_nano = 1672531200000_i128 * 1_000_000;
    let close_time = OffsetDateTime::from_unix_timestamp_nanos(close_time_nano).unwrap();
    dbg!(close_time);
}
