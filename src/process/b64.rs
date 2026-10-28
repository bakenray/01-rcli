use base64::{ engine::general_purpose::{URL_SAFE_NO_PAD, STANDARD}, Engine as _};
use crate::{get_reader, Base64Format};
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
        Base64Format::Standard => STANDARD.decode(&buf)?,
        Base64Format::UrlSafe => URL_SAFE_NO_PAD.decode(&buf)?,
    };

    Ok(decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_encode() {
      let input = "Cargo.toml";
      let format = Base64Format::Standard;
      assert!(process_encode(input, format).is_ok());
    }

    #[test]
    fn test_process_decode() {
      let input = "fixtures/b64.txt";
      let format = Base64Format::UrlSafe;
      process_decode(input, format).unwrap();
    }
}
