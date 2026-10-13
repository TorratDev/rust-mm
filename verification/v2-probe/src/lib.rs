pub fn add(a: u32, b: u32) -> u32 { a + b }

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    #[test]
    fn snapshot() { insta::assert_snapshot!("ok", "ok"); }
    proptest! {
        #[test]
        fn commutes(a in 0u32..1000, b in 0u32..1000) {
            prop_assert_eq!(super::add(a, b), super::add(b, a));
        }
    }
}
