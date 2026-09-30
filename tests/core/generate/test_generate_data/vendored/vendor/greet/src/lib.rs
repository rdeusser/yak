pub fn greeting() -> String {
    format!("hello from {}", env!("GREET_FROM_BUILD_SCRIPT"))
}
