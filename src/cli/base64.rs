use clap::{Parser, ValueEnum};
use super::verify_input_file; //从mod中引入verify_input_file

#[derive(Debug, Parser)]
pub enum Base64SubCommand {
    #[command(name = "encode", about = "Encode input to a base64 string")]
    Encode(Base64EncodeOpts),

    #[command(name = "decode", about = "Decode a base64 string to original input")]
    Decode(Base64DecodeOpts),
}

#[derive(Debug, Parser)]
pub struct Base64EncodeOpts {
    #[arg(short, long, value_parser = verify_input_file, default_value = "-")]
    pub input: String,

    #[arg(short, long, value_enum, default_value = "standard")]
    pub format: Base64Format,
}

#[derive(Debug, Parser)]
pub struct Base64DecodeOpts {
    #[arg(short, long, value_parser = verify_input_file, default_value = "-")]
    pub input: String,

    #[arg(short, long, value_enum, default_value = "standard")]
    pub format: Base64Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Base64Format {
    Standard,
    UrlSafe,
}
