/// Bridge lifecycle state, published via Watch for peripheral and LED tasks.
#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum BridgeState {
    /// Scanning for FTMS treadmills
    Scanning,
    /// Connecting to a candidate device
    Connecting,
    /// Actively bridging FTMS → RSC data
    Bridging,
}
