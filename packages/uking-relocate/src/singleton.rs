use uking_relocate_lib::{Env, Singleton, SingletonAlloc};

#[allow(dead_code)]
pub fn pause_menu_data_mgr(env: Env) -> SingletonAlloc {
    let size = 0x44808;
    let (create, ctor_invoke) = if env.is_1_6_0() {
        todo!()
    } else {
        ((0x0096b1cc, None), 0x0096b23c)
    };
    let rel_start = 0xaaaaaaa0; // TODO

    SingletonAlloc {
        id: Singleton::PauseMenuDataMgr,
        rel_start,
        size,
        create,
        ctor_invoke,
    }
}
