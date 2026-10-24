#[cfg(test)]
mod tests;

/// An unsigned integer as decimal text in a stack buffer — for numbers
/// written on every request (sizes, timestamps, status codes), where
/// `to_string()`/`format!` cost a trip through the formatting machinery and
/// a heap allocation each.
pub(crate) struct Decimal {
    digits: [u8; 39],
    start: usize,
}

impl Decimal {
    pub(crate) fn new(n: u128) -> Decimal {
        let mut decimal = Decimal { digits: [0; 39], start: 39 };
        // u128 division is a library call; nanosecond timestamps fit a u64
        // until 2554, so take the fast path whenever the value does.
        match u64::try_from(n) {
            Ok(mut n) => loop {
                decimal.start -= 1;
                decimal.digits[decimal.start] = b'0' + (n % 10) as u8;
                n /= 10;
                if n == 0 {
                    break;
                }
            },
            Err(_) => {
                let mut n = n;
                loop {
                    decimal.start -= 1;
                    decimal.digits[decimal.start] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 {
                        break;
                    }
                }
            }
        }
        decimal
    }

    pub(crate) fn as_str(&self) -> &str {
        // Only ASCII digits are ever written.
        std::str::from_utf8(&self.digits[self.start..]).unwrap_or("")
    }
}
