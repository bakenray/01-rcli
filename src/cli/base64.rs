use super::verify_file; //从mod中引入verify_input_file
use crate::CmdExecutor;
use clap::{Parser, ValueEnum};
use enum_dispatch::enum_dispatch;

#[enum_dispatch(CmdExecutor)]
#[derive(Debug, Parser)]
pub enum Base64SubCommand {
    #[command(name = "encode", about = "Encode input to a base64 string")]
    Encode(Base64EncodeOpts),

    #[command(name = "decode", about = "Decode a base64 string to original input")]
    Decode(Base64DecodeOpts),
}

#[derive(Debug, Parser)]
pub struct Base64EncodeOpts {
    #[arg(short, long, value_parser = verify_file, default_value = "-")]
    pub input: String,

    #[arg(short, long, value_enum, default_value = "standard")]
    pub format: Base64Format,
}

#[derive(Debug, Parser)]
pub struct Base64DecodeOpts {
    #[arg(short, long, value_parser = verify_file, default_value = "-")]
    pub input: String,

    #[arg(short, long, value_enum, default_value = "standard")]
    pub format: Base64Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Base64Format {
    Standard,
    UrlSafe,
}

impl CmdExecutor for Base64EncodeOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let ret = crate::process_encode(&self.input, self.format)?;
        println!("{}", ret);
        Ok(())
    }
}
impl CmdExecutor for Base64DecodeOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let ret = crate::process_decode(&self.input, self.format)?;
        // 解出来的是字节：可能是文本，也可能是图片之类的二进制。
        // 这里显式走 from_utf8，“base64 解得动”并不等于“内容是人类可读文本”。
        let text = String::from_utf8(ret)?;
        println!("{text}");
        Ok(())
    }
}
