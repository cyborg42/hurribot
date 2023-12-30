use std::fs::File;
use tracing::{error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;

/// 手续费（maker为0.02%，taker为0.05%）
const HANDLING_FEE_RATE_MAKER: f64 = 0.0002;
const HANDLING_FEE_RATE_TAKER: f64 = 0.0005;

// TODO: 币安会在0:00 8:00 16:00进行资金费率结算，若需支付资金费率则提前一分钟平仓并推迟10s建仓，若需收取资金费率则推迟一分钟平仓+建仓

fn main() {
    let log_name = format!("{}", chrono::Local::now().format("hurribot_%Y-%m-%d-%H:%M:%S"));
    let _logger_guard = init_log(&log_name);
    let chart = CandleChart::read_from_csv("./data/BTCUSDT_1h.csv", 3600);
    // chart.candles = chart.candles.into_iter().filter(|c|c.time > 1678996800).collect();
    let ratio = 1.;
    // 理论上止损比例及其他参数固定的情况下，ratio * leverage相等时，收益率和风险相同
    let mut strategy = GeoStrategy::new(10., ratio, 3600, 10., 0.998, 1.001);

    for (i, candle) in chart.candles.iter().enumerate() {
        if i % 100 == 0 {
            info!("{i}: current value: {}", strategy.value());
        }
        strategy.update(candle);
    }
    strategy.close(chart.candles.last().unwrap().close);
    let ret = strategy.capital / strategy.cost;
    info!(
        "ratio: {ratio}, add money: {}, captial: {}, return rate: {}, count: {}",
        strategy.cost, strategy.capital, ret, strategy.open_count
    );
}
// 等比定时开仓策略
#[derive(Debug, Clone)]
struct GeoStrategy {
    /// 杠杆
    leverage: f64,
    /// 开仓间隔
    interval: i64,
    /// 每次开仓占总资金比例
    ratio: f64,
    /// 若总资金不足则补充到此值
    supply: f64,
    /// 止损比例
    stop_loss_percentage: f64,
    /// 超过间隔后按该比例止盈
    take_profit_percentage: f64,
    /// 当前持仓
    position: Option<Contract>,
    /// 当前资金
    capital: f64,
    /// 总成本（补充资金总值）
    cost: f64,
    /// 开单次数
    open_count: i64,
    /// 上次开单时间
    last_time: i64,
}

impl GeoStrategy {
    fn new(
        leverage: f64,
        ratio: f64,
        interval: i64,
        supply: f64,
        stop_loss_percentage: f64,
        take_profit_percentage: f64,
    ) -> Self {
        if take_profit_percentage < 1. + HANDLING_FEE_RATE_MAKER {
            warn!(
                "take profit percentage is too low, can't take profit unless it is greater than {}",
                1. + HANDLING_FEE_RATE_MAKER
            );
        }
        Self {
            leverage,
            interval,
            ratio,
            supply,
            stop_loss_percentage,
            take_profit_percentage,
            position: None,
            capital: 0.,
            cost: 0.,
            open_count: 0,
            last_time: -10000000000,
        }
    }
    fn update(&mut self, candle: &CandleData) {
        if let Some(contract) = self.position.take() {
            if let Some(r) = contract.liquidate(candle.low) {
                // 止损或强制平仓
                self.capital += r;
            } else if contract.open_time + self.interval <= candle.time
                && candle.close > contract.entry_price * self.take_profit_percentage
            {
                // 超过间隔后按比例止盈，否则继续持有该仓位
                self.capital += contract.close(candle.close);
            } else {
                self.position = Some(contract);
            }
        }
        if self.position.is_some() || self.last_time + self.interval > candle.time {
            // 只有空仓且超过间隔后才开仓
            return;
        }
        self.last_time = candle.time;
        if self.capital < self.supply {
            self.cost += self.supply - self.capital;
            self.capital = self.supply;
        }
        self.open_count += 1;
        self.position = Some(Contract::open(
            candle.close,
            self.capital * self.ratio,
            self.leverage,
            candle.time,
            Some(self.stop_loss_percentage * candle.close),
        ));
        self.capital -= self.capital * self.ratio;
    }
    fn close(&mut self, price: f64) {
        if let Some(offer) = self.position.take() {
            self.capital += offer.close(price);
        }
    }
    fn value(&self) -> f64 {
        if let Some(offer) = &self.position {
            offer.margin + self.capital
        } else {
            self.capital
        }
    }
}

#[derive(Debug, Clone)]
struct Contract {
    /// 保证金
    margin: f64,
    /// 开仓价格
    entry_price: f64,
    /// 开仓时间
    open_time: i64,
    /// 强平价格（维持保证金 = 0.4% * 名义价值）
    liq_price: f64,
    /// 盈亏平衡价格
    break_even_price: f64,
    /// 合约数量（合约数量 * 现价 = 名义价值）
    amount: f64,
    /// 杠杆
    leverage: f64,
    /// 止损价格（需大于强平价格）
    stop_loss: Option<f64>,
}
impl Contract {
    fn open(
        entry_price: f64,
        offered_balance: f64,
        leverage: f64,
        open_time: i64,
        mut stop_loss: Option<f64>,
    ) -> Self {
        // 初始保证金 + 手续费消耗 = 提供资金，手续费消耗 = 初始保证金 * 杠杆 * 手续费率
        // 由上面两个公式可得：初始保证金 = 提供资金 / (1 + 杠杆 * 手续费率)
        let margin = offered_balance / (1. + leverage * HANDLING_FEE_RATE_MAKER);
        let liq_price = entry_price * (1. - 0.6 / leverage);
        let amount = margin * leverage / entry_price;
        let break_even_price = (1. + HANDLING_FEE_RATE_MAKER) * entry_price;
        if let Some(sl) = stop_loss {
            if sl < liq_price {
                error!("stop loss price is lower than liquidation price");
                stop_loss = None;
            }
        }
        Self {
            margin,
            entry_price,
            open_time,
            liq_price,
            break_even_price,
            amount,
            leverage,
            stop_loss,
        }
    }
    // 止损平仓或强制平仓，强制平仓有15%的强平费用，所以尽量确保不要强平
    fn liquidate(&self, price: f64) -> Option<f64> {
        if let Some(stop_loss) = self.stop_loss {
            if price < stop_loss {
                return Some(self._cover(stop_loss));
            }
        }
        if price < self.liq_price {
            return Some(self._cover(self.liq_price) * 0.85);
        }
        None
    }
    fn close(&self, price: f64) -> f64 {
        if let Some(r) = self.liquidate(price) {
            return r;
        }
        self._cover(price)
    }
    fn _cover(&self, price: f64) -> f64 {
        self.amount * (price - self.entry_price) + self.margin - self.amount * price * HANDLING_FEE_RATE_MAKER
    }
}

#[derive(Debug)]
struct CandleChart {
    /// k线间隔（秒）
    interval: i64,
    candles: Vec<CandleData>,
}

impl CandleChart {
    fn new(interval: i64) -> Self {
        Self {
            interval,
            candles: vec![],
        }
    }
    fn read_from_csv(file: &str, interval: i64) -> Self {
        let file = File::open(file).unwrap();
        let mut csv = csv::Reader::from_reader(file);
        let mut chart = Self::new(interval);
        for d in csv.records() {
            let d = d.unwrap();
            let time = d.get(0).unwrap().parse::<i64>().unwrap();
            let open = d.get(3).unwrap().parse::<f64>().unwrap();
            let high = d.get(4).unwrap().parse::<f64>().unwrap();
            let low = d.get(5).unwrap().parse::<f64>().unwrap();
            let close = d.get(6).unwrap().parse::<f64>().unwrap();
            let volume = d.get(8).unwrap().parse::<f64>().unwrap();
            chart.candles.push(CandleData {
                open,
                high,
                low,
                close,
                volume,
                time,
            });
        }
        chart.candles.reverse();
        chart
    }
}

#[derive(Debug)]
struct CandleData {
    open: f64,
    close: f64,
    high: f64,
    low: f64,
    volume: f64,
    time: i64,
}

#[test]
fn offer_test() {
    let offer = Contract::open(100., 100., 100., 100, Some(99.9));
    println!("{:?}", offer);
    println!("{:?}", offer.liquidate(9.));
}

fn init_log(file_name: &str) -> WorkerGuard {
    use tracing::Level;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::fmt::time::FormatTime;
    use tracing_subscriber::FmtSubscriber;

    struct UtcOffset;
    impl FormatTime for UtcOffset {
        fn format_time(
            &self,
            w: &mut tracing_subscriber::fmt::format::Writer<'_>,
        ) -> std::fmt::Result {
            let now = chrono::Local::now();
            write!(w, "{}", now.format("%Y-%m-%d %H:%M:%S"))
        }
    }
    let file_appender = tracing_appender::rolling::daily("./logs", file_name);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_writer(non_blocking)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_timer(UtcOffset)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("logger started");
    _guard
}
