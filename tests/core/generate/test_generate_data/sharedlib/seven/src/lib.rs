unsafe extern "C" {
    #[link_name = "seven"]
    fn c_seven() -> i32;
}

pub fn seven() -> i32 {
    unsafe { c_seven() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_seven() {
        assert_eq!(super::seven(), 7);
    }
}
