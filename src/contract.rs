use time::OffsetDateTime;
use tracing::error;

/// 手续费率（maker为0.02%，taker为0.05%，随VIP等级变化）
pub const HANDLING_FEE_RATE_MAKER: f64 = 0.0002;
pub const HANDLING_FEE_RATE_TAKER: f64 = 0.0005;
#[derive(Debug, Clone)]
pub struct Contract {
    pub is_bull: bool,
    /// 保证金
    pub margin: f64,
    /// 开仓价格
    pub entry_price: f64,
    /// 开仓时间
    pub open_time: OffsetDateTime,
    /// 强平价格（维持保证金 = 0.4% * 初始名义价值）
    pub liq_price: f64,
    /// 合约数量（合约数量 * 现价 = 名义价值）
    pub amount: f64,
    /// 杠杆
    pub leverage: f64,
    /// 止损价格（需大于强平价格）
    pub stop_loss: Option<f64>,
}
impl Contract {
    pub fn open(
        is_bull: bool,
        entry_price: f64,
        offered_balance: f64,
        leverage: f64,
        open_time: OffsetDateTime,
        mut stop_loss: Option<f64>,
    ) -> Self {
        // 初始保证金 + 手续费消耗 = 提供资金；手续费消耗 = 初始保证金 * 杠杆 * 手续费率
        // 由上面两个公式可得：初始保证金 = 提供资金 / (1 + 杠杆 * 手续费率)
        let margin = offered_balance / (1. + leverage * HANDLING_FEE_RATE_MAKER);
        let liq_price = if is_bull {
            entry_price * (1. - 1. / leverage) + entry_price * 0.004
        } else {
            entry_price * (1. + 1. / leverage) - entry_price * 0.004
        };
        let amount = margin * leverage / entry_price;
        if let Some(sl) = stop_loss {
            if (is_bull && sl < liq_price) || (!is_bull && sl > liq_price) {
                error!("stop loss price exceeds liquidation price");
                stop_loss = None;
            }
        }
        Self {
            is_bull,
            margin,
            entry_price,
            open_time,
            liq_price,
            amount,
            leverage,
            stop_loss,
        }
    }
    /// 止损平仓或强制平仓，强制平仓有15%的强平费用，所以尽量确保不要强平
    pub fn liquidate(&self, price: f64) -> Option<f64> {
        if let Some(stop_loss) = self.stop_loss {
            if (self.is_bull && price < stop_loss) || (!self.is_bull && price > stop_loss) {
                return Some(self.cover(stop_loss));
            }
        }
        if (self.is_bull && price < self.liq_price) || (!self.is_bull && price > self.liq_price) {
            return Some(self.cover(self.liq_price) * 0.85);
        }
        None
    }
    pub fn close(&self, price: f64) -> f64 {
        if let Some(r) = self.liquidate(price) {
            return r;
        }
        self.cover(price)
    }
    /// 理想状态是只做挂单且不会被穿透，但实盘会有这两种风险
    fn cover(&self, price: f64) -> f64 {
        if self.is_bull {
            self.amount * (price - self.entry_price) + self.margin
                - self.amount * price * HANDLING_FEE_RATE_MAKER
        } else {
            self.amount * (self.entry_price - price) + self.margin
                - self.amount * price * HANDLING_FEE_RATE_MAKER
        }
    }
}

#[test]
fn contract_test() {
    let offer = Contract::open(
        true,
        100.,
        100.,
        100.,
        OffsetDateTime::from_unix_timestamp(0).unwrap(),
        Some(99.9),
    );
    println!("{:?}", offer);
    println!("{:?}", offer.liquidate(9.));
}
