use crate::{get_reader, Base64Format};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use std::io::Read;

pub fn process_encode(input: &str, format: Base64Format) -> anyhow::Result<String> {
    // 这里if和else提供了不同的数据类型
    // if返回stdin，else返回fail
    // 寻找他们的共同点，reader
    // 升级结构为Box, 同时提升到一个dynamic trai中
    // 使用box来消除者两个不同的返回
    let mut reader = get_reader(input)?;
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;

    let encoded = match format {
        Base64Format::Standard => STANDARD.encode(&buf),
        Base64Format::UrlSafe => URL_SAFE_NO_PAD.encode(&buf),
    };

    Ok(encoded)
}

pub fn process_decode(input: &str, format: Base64Format) -> anyhow::Result<Vec<u8>> {
    let mut reader = get_reader(input)?;
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;
    //avoid accidental newlines
    let buf = buf.trim();

    let decode = match format {
        Base64Format::Standard => STANDARD.decode(buf)?,
        Base64Format::UrlSafe => URL_SAFE_NO_PAD.decode(buf)?,
    };

    Ok(decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_encode() {
        // Cargo.toml 以 "[package]" 开头，前 9 字节的 base64 是固定值：
        // 断言内容而不是 is_ok()，这样文件被改坏时测试会真的失败
        let got = process_encode("Cargo.toml", Base64Format::Standard).unwrap();
        assert!(got.starts_with("W3BhY2thZ2Vd"), "{got}");
    }

    #[test]
    fn test_process_decode() {
        // 夹具 b64.txt 是 "Hello, rcli base64!" 的 url-safe 无填充编码；
        // 以前这条只写了 unwrap()（只能测“没报错”），夹具被覆盖成别的内容时依旧绿
        let got = process_decode("fixtures/b64.txt", Base64Format::UrlSafe).unwrap();
        assert_eq!(String::from_utf8(got).unwrap(), "Hello, rcli base64!");
    }

    #[test]
    fn test_process_decode_rejects_wrong_alphabet() {
        // url-safe 无填充的内容用 standard 解：长度不是 4 的倍数，必报填充错误
        assert!(process_decode("fixtures/b64.txt", Base64Format::Standard).is_err());
    }
}
