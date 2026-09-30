use std::process::Command;

#[test]
fn runs_the_binary() {
    let output = Command::new(env!("CARGO_BIN_EXE_probe")).output().unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "probe\n");
}
