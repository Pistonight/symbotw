use uking_relocate_lib::singleton::{CreateByteCode as C, Singleton, SingletonInfo};
use uking_relocate_lib::Env;

#[allow(dead_code)]
pub fn pause_menu_data_mgr(env: Env) -> SingletonInfo {
    let rel_start = 0xaaaaaaa0; // TODO, based on env
    let size = 0x44808; // should be the same for all envs
                        //
    let create_bytecode = if env.is_1_6_0() {
        todo!()
    } else {
        vec![
            C::Enter(0x0096b1cc),
            C::ExecuteUntil(0x0096b200),
            C::Allocate,
            C::Jump(0x0096b204),
            C::ExecuteUntil(0x0096b218),
            // skip the Disposer ctor
            C::Jump(0x0096b21c),
            C::ExecuteToReturn,
        ]
    };

    SingletonInfo::new(
        Singleton::PauseMenuDataMgr,
        rel_start,
        size,
        create_bytecode,
    )
}
