use tracing::{error, info, warn};
use super::Strategy;
use crate::{
    candle_chart::CandleData,
    contract::{Contract, HANDLING_FEE_RATE_MAKER, HANDLING_FEE_RATE_TAKER},
};

struct RollOnceStratege {
    is_bull: bool,
    config: RollConfig,
    contract: Option<Contract>,
}



struct RollConfig {

}