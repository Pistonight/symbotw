use deku::{DekuRead, DekuWrite};
use serde::{Deserialize, Serialize};

/// Environment settings
mod env;
/// Serialization and deserialization of the program
mod pack;
pub use pack::*;

pub use env::Env;

/// Memory information of a program at runtime
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
pub struct Program {
    /// Environment for the program
    pub env: Env,

    singleton_len: u32, // required for serialization

    /// Allocation info for the singletons. These are derived from the game
    #[deku(count = "singleton_len")]
    singletons: Vec<SingletonAlloc>,

    /// Physical address of the start of the program region (where nnrtld is loaded), must be page aligned (4KB)
    pub program_start: u64,

    program_regions_len: u32, // required for serialization

    /// Regions of the program to load.
    #[deku(count = "program_regions_len")]
    program_regions: Vec<ProgramRegion>,
}

impl Program {
    /// Get the single allocations
    pub fn singletons(&self) -> &[SingletonAlloc] {
        &self.singletons
    }

    /// Get the program regions
    pub fn regions(&self) -> &[ProgramRegion] {
        &self.program_regions
    }
}

/// Builder for a program
///
/// The binary serialization requires that the length
/// fields are set correctly for Vecs. This builder
/// is used to ensure that
pub struct ProgramBuilder {
    env: Env,
    singletons: Vec<SingletonAlloc>,
    program_start: u64,
    program_regions: Vec<ProgramRegion>,
}

impl ProgramBuilder {
    /// Create a new builder and set the environment
    pub fn new(env: Env) -> Self {
        Self {
            env,
            singletons: Vec::new(),
            program_start: 0,
            program_regions: Vec::new(),
        }
    }

    /// Set the singleton allocation info
    pub fn singletons(mut self, singletons: Vec<SingletonAlloc>) -> Self {
        self.singletons = singletons;
        self
    }

    /// Set the program regions
    pub fn program(mut self, start: u64, regions: Vec<ProgramRegion>) -> Self {
        self.program_start = start;
        self.program_regions = regions;
        self
    }

    /// Build the program
    pub fn build(self) -> Program {
        Program {
            env: self.env,
            singleton_len: self.singletons.len() as u32,
            singletons: self.singletons,
            program_start: self.program_start,
            program_regions_len: self.program_regions.len() as u32,
            program_regions: self.program_regions,
        }
    }
}

/// One contiguous region of the program memory
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
pub struct ProgramRegion {
    /// Start of the region relative to the program_start, must be page aligned (4KB)
    pub rel_start: u32,
    /// Permission of the region
    ///  - 0x1: Execute
    ///  - 0x2: Write
    ///  - 0x4: Read
    pub permissions: u32,
    // /// Length of the data in the region (for serialization only)
    data_len: u32,
    /// Data of the region, must be page aligned (4KB)
    #[deku(count = "data_len")]
    data: Vec<u8>,
}

impl ProgramRegion {
    pub fn new(rel_start: u32, permissions: u32, data: Vec<u8>) -> Self {
        let data_len = data.len() as u32;
        Self {
            rel_start,
            permissions,
            data_len,
            data,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

/// Allocation and initialization info for a singleton
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
pub struct SingletonAlloc {
    /// Identifier of the singleton
    pub id: Singleton,
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
    pub ctor_invoke: u32,
}

/// Singleton identifiers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
#[deku(id_type = "u8")]
pub enum Singleton {
    #[deku(id = 0x01)]
    PauseMenuDataMgr,
}
