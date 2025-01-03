use deku::{DekuRead, DekuWrite};
use serde::{Deserialize, Serialize};

/// Environment to simulate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
#[deku(id_type = "u8")]
pub enum Env {
    /// ver 1.5.0, no DLC
    #[deku(id = 0x01)]
    X150,
    /// ver 1.6.0, no DLC
    #[deku(id = 0x02)]
    X160,
    /// ver 1.5.0 + DLC ver 3
    #[deku(id = 0x03)]
    X150DLC,
    /// ver 1.6.0 + DLC ver 3
    #[deku(id = 0x04)]
    X160DLC,
}

impl Env {
    pub const fn is_1_6_0(&self) -> bool {
        matches!(self, Env::X160 | Env::X160DLC)
    }
}
