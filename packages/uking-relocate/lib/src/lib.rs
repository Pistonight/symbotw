mod env;
mod singletons;

use env::Env;
use serde::{Deserialize, Serialize};
use singletons::SingletonAlloc;


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub env: Env,
    /// Physical address of the start of the stack region, must be page aligned (4KB)
    pub stack_start: u64,
    /// Size of the stack region. Currently 400KB
    pub stack_size: u32,

    /// Physical address of the start of the heap region, must be page aligned (4KB)
    pub heap_start: u64,
    /// Max size of the heap region. Currently 400KB
    pub heap_size: u32,
    /// Allocation info for the singletons. These are derived from the game
    pub singletons: Vec<SingletonAlloc>,
    
    /// Physical address of the start of the program region (where nnrtld is loaded), must be page aligned (4KB)
    pub program_start: u64,

    /// Regions of the program to load. Undefined regions will be zeroed out
    pub program_regions: Vec<ProgramRegion>,
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramRegion {
    /// Start of the region relative to the program_start, must be page aligned (4KB)
    pub rel_start: u32,
    /// Data of the region, must be page aligned (4KB)
    pub data: Vec<u8>,
}

