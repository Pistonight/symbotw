use serde::{Deserialize, Serialize};

/// Environment to simulate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Env {
    /// ver 1.5.0, no DLC
    X150,
    /// ver 1.6.0, no DLC
    X160,
    /// ver 1.5.0 + DLC ver 3
    X150DLC,
    /// ver 1.6.0 + DLC ver 3
    X160DLC
}

impl Env {
    pub const fn is_1_6_0(&self) -> bool {
        matches!(self, Env::X160 | Env::X160DLC)
    }
}
