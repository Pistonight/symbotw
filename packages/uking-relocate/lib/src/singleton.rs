use deku::{DekuRead, DekuWrite};
use serde::{Deserialize, Serialize};

use crate::Env;

/// Trait implemented by CPU for creating singletons
pub trait SingletonCreator {
    type Error;

    /// Set PC relative to the start of the main module without doing anything else
    /// (i.e. the next instruction to execute is at `pc`)
    fn set_main_rel_pc(&mut self, pc: u32) -> Result<(), Self::Error>;

    /// Enter the singleton creation function
    ///
    /// The CPU should treat `target` as the start of a function, sets
    /// up the stack pointer, and jumps to the target.
    /// When returning, PC should be pointing at target
    fn enter(&mut self, target: u32) -> Result<(), Self::Error>;

    /// Execute the program, and return when the next instruction
    /// is at `target`. Target is relative to the start of the main module
    fn execute_until(&mut self, target: u32) -> Result<(), Self::Error>;

    /// Simulate allocation of the singleton
    ///
    /// `rel_start` is where the singleton would be allocated relative to the heap start,
    /// and size is the size. The implementation should put the singleton pointer (i.e.
    /// address of the singleton in physical memory) into X0
    fn allocate(&mut self, rel_start: u32, size: u32) -> Result<(), Self::Error>;

    /// Execute until jumping out of the singleton creation function
    fn execute_to_return(&mut self) -> Result<(), Self::Error>;

    /// Stop the execution, singleton is done created
    fn stop(&mut self) -> Result<(), Self::Error>;
}

/// Allocation and initialization info for a singleton
///
/// To encapsulate the singleton allocation and initialization process,
/// the [`SingletonCreator`] trait is provided. At runtime,
/// call `create_instance` to allocate and initialize the singleton
/// with a creator
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
pub struct SingletonInfo {
    /// Identifier of the singleton
    pub id: Singleton,
    /// Start of the allocation relative to the heap_start
    pub rel_start: u32,
    /// Size of the object
    pub size: u32,

    /// Byte code to create the singleton
    create_bytecode_len: u32,
    #[deku(count = "create_bytecode_len")]
    create_bytecode: Vec<CreateByteCode>,
}

impl SingletonInfo {
    pub fn new(
        id: Singleton,
        rel_start: u32,
        size: u32,
        create_bytecode: Vec<CreateByteCode>,
    ) -> Self {
        let create_bytecode_len = create_bytecode.len() as u32;
        Self {
            id,
            rel_start,
            size,
            create_bytecode_len,
            create_bytecode,
        }
    }
    pub fn create_instance<C: SingletonCreator>(&self, creator: &mut C) -> Result<(), C::Error> {
        for bytecode in &self.create_bytecode {
            match bytecode {
                CreateByteCode::Enter(target) => creator.enter(*target)?,
                CreateByteCode::ExecuteUntil(target) => creator.execute_until(*target)?,
                CreateByteCode::Allocate => creator.allocate(self.rel_start, self.size)?,
                CreateByteCode::Jump(target) => creator.set_main_rel_pc(*target)?,
                CreateByteCode::ExecuteToReturn => creator.execute_to_return()?,
                CreateByteCode::Return => creator.stop()?,
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
#[deku(id_type = "u8")]
pub enum CreateByteCode {
    /// Enter the singleton creation function
    #[deku(id = 0x01)]
    Enter(u32),
    /// Execute the program, and return when the next instruction
    /// is at the target
    #[deku(id = 0x02)]
    ExecuteUntil(u32),

    /// Simulate allocation of the singleton (rel_start to heap, size)
    #[deku(id = 0x03)]
    Allocate,

    /// Set the PC to relative to the start of the main module, without
    /// doing anything else
    #[deku(id = 0x04)]
    Jump(u32),

    /// Execute the program until jumping out of the singleton creation function
    #[deku(id = 0x05)]
    ExecuteToReturn,

    /// Stop the execution, singleton is done created
    #[deku(id = 0x06)]
    Return,
}

/// Singleton identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DekuRead, DekuWrite)]
#[deku(id_type = "u8")]
pub enum Singleton {
    #[deku(id = 0x01)]
    PauseMenuDataMgr,
}

impl Singleton {
    /// Get offset of the instance global variable of this singleton in the main module
    /// (i.e. program_start + offset is a static Singleton*)
    pub const fn get_main_offset(self, env: Env) -> u32 {
        match self {
            Singleton::PauseMenuDataMgr => {
                if env.is_1_6_0() {
                    0x2ca6d50
                } else {
                    0x25d75b8
                }
            }
        }
    }
}
