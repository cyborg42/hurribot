use tracing::{error, info, warn};

/// 手续费率（maker为0.02%，taker为0.05%，随VIP等级变化）
pub const HANDLING_FEE_RATE_MAKER: f64 = 0.0002;
pub const HANDLING_FEE_RATE_TAKER: f64 = 0.0005;
#[derive(Debug, Clone)]
pub struct Contract {
    /// 保证金
    pub margin: f64,
    /// 开仓价格
    pub entry_price: f64,
    /// 开仓时间
    pub open_time: i64,
    /// 强平价格（维持保证金 = 0.4% * 初始名义价值）
    pub liq_price: f64,
    /// 盈亏平衡价格（计算了开仓和平仓的手续费，币安计算了开仓手续费）
    pub break_even_price: f64,
    /// 合约数量（合约数量 * 现价 = 名义价值）
    pub amount: f64,
    /// 杠杆
    pub leverage: f64,
    /// 止损价格（需大于强平价格）
    pub stop_loss: Option<f64>,
}
impl Contract {
    pub fn open(
        entry_price: f64,
        offered_balance: f64,
        leverage: f64,
        open_time: i64,
        mut stop_loss: Option<f64>,
    ) -> Self {
        // 初始保证金 + 手续费消耗 = 提供资金；手续费消耗 = 初始保证金 * 杠杆 * 手续费率
        // 由上面两个公式可得：初始保证金 = 提供资金 / (1 + 杠杆 * 手续费率)
        let margin = offered_balance / (1. + leverage * HANDLING_FEE_RATE_MAKER);
        let liq_price = entry_price * (1. - 1. / leverage) + entry_price * 0.004;
        let amount = margin * leverage / entry_price;
        let break_even_price = (1. + HANDLING_FEE_RATE_MAKER * 2.) * entry_price;
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
    /// 止损平仓或强制平仓，强制平仓有15%的强平费用，所以尽量确保不要强平
    pub fn liquidate(&self, price: f64) -> Option<f64> {
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
    pub fn close(&self, price: f64) -> f64 {
        if let Some(r) = self.liquidate(price) {
            return r;
        }
        self._cover(price)
    }
    /// 理想状态是只做挂单且不会被穿透，但实盘会有这两种风险
    fn _cover(&self, price: f64) -> f64 {
        self.amount * (price - self.entry_price) + self.margin
            - self.amount * price * HANDLING_FEE_RATE_MAKER
    }
}