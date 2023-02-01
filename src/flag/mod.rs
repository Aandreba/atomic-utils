/// Multiple producer - Multiple consumer flag
pub mod mpmc;

/// Multiple producer - Single consumer flag. Can also be used as a SPSC flag
pub mod mpsc;

/// Single producer - Single consumer flag.
#[deprecated(since = "0.4.1", note = "use `mpsc` instead")]
pub mod spsc {
    pub use super::mpsc::*;
}

// Legacy
pub use mpmc::*;