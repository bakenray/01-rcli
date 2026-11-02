use rand::seq::{IndexedRandom, SliceRandom};

//const必须指定类型
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const NUMBER: &[u8] = b"0123456789";
const SYMBOL: &[u8] = b"!@#$%^&*_";

pub fn process_genpass(
    length: u8,
    upper: bool,
    lower: bool,
    number: bool,
    symbol: bool,
) -> anyhow::Result<String> {
    let mut rng = rand::rng();
    let mut password = Vec::new();
    let mut chars = Vec::new();

    // 启用的字符类数量就是“必须占掉的保证位数”。
    // 下面那圈 `length - password.len()` 是 u8 减法，length 比保证位数小时会下溢：
    // debug 构建直接 panic，release 构建（默认关溢出检查）则静默回绕成一个超长的密码。
    // 所以在这里提前拦住，把它变成正常的 Err。
    let required = [upper, lower, number, symbol]
        .iter()
        .filter(|on| **on)
        .count();
    anyhow::ensure!(
        required > 0,
        "at least one character class (--upper/--lower/--number/--symbol) must be enabled"
    );
    anyhow::ensure!(
        length as usize >= required,
        "password length {length} is too short: {required} character classes are enabled and each \
         needs one slot, so the minimum length is {required}"
    );

    if upper {
        chars.extend_from_slice(UPPER);
        password.push(*UPPER.choose(&mut rng).expect("UPPER won't be empty"));
    }
    if lower {
        chars.extend_from_slice(LOWER);
        password.push(*LOWER.choose(&mut rng).expect("LOWER won't be empty"));
    }
    if number {
        chars.extend_from_slice(NUMBER);
        password.push(*NUMBER.choose(&mut rng).expect("NUMBER won't be empty"));
    }
    if symbol {
        chars.extend_from_slice(SYMBOL);
        password.push(*SYMBOL.choose(&mut rng).expect("SYMBOL won't be empty"));
    }

    for _ in 0..(length - password.len() as u8) {
        let c = chars
            .choose(&mut rng)
            .expect("chars won't be empty in this context");
        password.push(*c);
    }

    password.shuffle(&mut rng);

    let password = String::from_utf8(password)?;

    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genpass_too_short_is_error_not_panic() {
        // 回归：4 个字符类全开却只给 3 位。
        // 修复前：debug 版 panic(attempt to subtract with overflow)，release 版静默输出 259 个字符。
        let err = process_genpass(3, true, true, true, true).unwrap_err();
        assert!(err.to_string().contains("too short"), "unexpected: {err}");
    }

    #[test]
    fn test_genpass_no_class_enabled_is_error() {
        // 一个字符类都没开时池子是空的，choose() 会 panic，也必须提前报错
        assert!(process_genpass(12, false, false, false, false).is_err());
    }

    #[test]
    fn test_genpass_returns_requested_length() {
        let p = process_genpass(16, true, true, true, true).unwrap();
        assert_eq!(p.chars().count(), 16, "{p}");
    }

    #[test]
    fn test_genpass_covers_every_enabled_class() {
        // “先各取一个 + shuffle”这两步存在的意义就靠这个断言来守
        let p = process_genpass(12, true, true, true, true).unwrap();
        assert!(p.chars().any(|c| c.is_ascii_uppercase()), "{p}");
        assert!(p.chars().any(|c| c.is_ascii_lowercase()), "{p}");
        assert!(p.chars().any(|c| c.is_ascii_digit()), "{p}");
        assert!(p.bytes().any(|b| SYMBOL.contains(&b)), "{p}");
    }

    #[test]
    fn test_genpass_minimal_length_is_exactly_required() {
        // 只开 number + symbol（默认的两个开关），最短合法长度就是 2
        assert!(process_genpass(1, false, false, true, true).is_err());
        let p = process_genpass(2, false, false, true, true).unwrap();
        assert_eq!(p.chars().count(), 2, "{p}");
        assert!(p.bytes().any(|b| NUMBER.contains(&b)), "{p}");
        assert!(p.bytes().any(|b| SYMBOL.contains(&b)), "{p}");
    }
}
