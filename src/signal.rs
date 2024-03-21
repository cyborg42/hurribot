pub struct Signal {
    pub signal_type: SignalType,
    pub symbol: String,
    pub value: f64,
}

pub enum SignalType {
    Roll,
    Grid,
}