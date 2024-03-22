#[derive(Debug)]
pub struct Signal {
    pub signal_type: SignalType,
    pub symbol: String,
    pub value: f64,
}
#[derive(Debug)]
pub enum SignalType {
    Roll,
    Grid,
}
