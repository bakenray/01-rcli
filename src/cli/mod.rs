mod csv;
mod genpass;
mod base64;

use std::{path::Path};
use clap::Parser;
use self::{csv::CsvOpts, genpass::GenPassOpts};
//使用self 是因为crate中也有同名的csv，为了避免歧义，使用selft前缀
pub use self::{csv::OutputFormat, base64::{ Base64SubCommand, Base64Format }};

#[derive(Debug, Parser)]
#[command(name = "rcli", version, author, about, long_about = None)]
pub struct Opts {
    #[command(subcommand)]
    pub cmd: SubCommand,
}

#[derive(Debug, Parser)]
pub enum SubCommand {
    #[command(name = "csv", about = "Show CSV, or convert csv to other formats")]
    Csv(CsvOpts),

    #[command(name = "genpass", about = "Generate a random password")]
    GenPass(GenPassOpts),

    #[command(subcommand)]
    Base64(Base64SubCommand),
}

fn verify_input_file(filename: &str) -> Result<String, &'static str> {
  if filename == "-" || Path::new(filename).exists() {
      Ok(filename.into())
  } else {
      Err("File does not exist")
  }
}

//单元测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_input_file() {
        assert_eq!(verify_input_file("-"), Ok("-".into()));
        assert_eq!(verify_input_file("*"), Err("File does not exist"));
        assert_eq!(verify_input_file("Cargo.toml"), Ok("Cargo.toml".into()));
        assert_eq!(verify_input_file("not-exist"), Err("File does not exist"));
    }
}