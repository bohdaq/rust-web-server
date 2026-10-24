use crate::ext::decimal::Decimal;

#[test]
fn matches_to_string() {
    for n in [0u128, 1, 9, 10, 99, 100, 200, 1024, 1_048_576, 1_767_222_000_000_000_000,
              u64::MAX as u128 - 1, u64::MAX as u128, u64::MAX as u128 + 1, u128::MAX] {
        assert_eq!(n.to_string(), Decimal::new(n).as_str(), "{}", n);
    }
}

#[test]
fn every_power_of_ten_boundary() {
    let mut n: u128 = 1;
    while let Some(next) = n.checked_mul(10) {
        for m in [n - 1, n, n + 1] {
            assert_eq!(m.to_string(), Decimal::new(m).as_str());
        }
        n = next;
    }
}
