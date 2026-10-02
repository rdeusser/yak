use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let include = env::var("DEP_ANSWER_INCLUDE").unwrap();
    let data_dir = env::var("DEP_ANSWER_DATA_DIR").unwrap();
    let answer = fs::read_to_string(Path::new(&include).join("answer.h")).unwrap();
    let name = fs::read_to_string(Path::new(&data_dir).join("name.txt")).unwrap();
    println!("cargo::rustc-env=ANSWER={answer} ({name})");
}
