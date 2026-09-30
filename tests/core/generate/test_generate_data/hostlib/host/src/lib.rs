unsafe extern "C" {
    fn host_answer() -> i32;
}

pub fn answer() -> i32 {
    unsafe { host_answer() }
}
