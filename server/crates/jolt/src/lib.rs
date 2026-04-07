//! Safe Rust wrapper around Jolt Physics.

pub use jolt_sys;

pub fn smoke_test() {
    unsafe {
        let ok = jolt_sys::JPH_Init();
        assert!(ok, "JPH_Init failed");
        jolt_sys::JPH_Shutdown();
    }
    println!("Jolt Physics linked successfully!");
}
