use serde::{Deserialize, Serialize};


/// Allocation and initialization info for a singleton
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SingletonAlloc {
    /// Identifier of the singleton
    pub name: String,
    /// Start of the allocation relative to the heap_start
    pub rel_start: u32,
    /// Size of the object
    pub size: u32,
    /// Range of the instructions to run to create the singleton. The end is exclusive.
    /// 
    /// The CPU will set up SP, then jump to the start of the range.
    /// If no end is provided, it will execute until RET
    pub create: (u32, Option<u32>),
    /// Address of the insturction that BLs to the constructor
    /// When constructing the singleton at runtime,
    /// CPU will inject the singleton address into X0
    pub ctor_invoke: u32
}

impl SingletonAlloc {
    #[cfg(feature = "loader-data")]
    pub fn make_pause_menu_data_mgr(env: crate::env::Env) -> Self {
        let name = "uking::ui::PauseMenuDataMgr".to_string();
        let size = 0x44808;
        let (create, ctor_invoke) = if env.is_1_6_0() {
            todo!()
        } else {
            ((0x0096b1cc, None), 0x0096b23c)
        };
        let rel_start = 0xaaaaaaa0;// TODO

        Self {
            name,
            rel_start,
            size,
            create,
            ctor_invoke
        }
    }
}
