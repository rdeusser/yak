pub fn double(x: i32) -> i32 {
    x * 2
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_its_data() {
        let data = std::fs::read_to_string("data.txt").unwrap();
        assert_eq!(data.lines().next(), Some("ok"));
        assert_eq!(super::double(2), 4);
    }
}
