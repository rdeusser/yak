fn main() {
    println!("probe");
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_the_example_binary() {
        let name = format!("hello{}", std::env::consts::EXE_SUFFIX);
        let path = probe::profile_dir().join("examples").join(name);
        assert!(path.exists(), "{} is missing", path.display());
    }
}
