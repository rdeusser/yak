// A call through the `C-unwind` ABI makes rustc require that a binary links
// this crate with the panic strategy that the crate was compiled with.
extern "C-unwind" fn noop() {}

#[inline(never)]
fn call(f: extern "C-unwind" fn()) {
    f()
}

pub fn greeting() -> String {
    call(noop);
    format!("hello from {}", env!("GREET_FROM_BUILD_SCRIPT"))
}
