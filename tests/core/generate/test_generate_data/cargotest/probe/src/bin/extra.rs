#[cfg(not(feature = "extra"))]
compile_error!("the extra binary needs the extra feature");

fn main() {}
