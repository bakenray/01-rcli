mod csv;
mod genpass;
mod base64;
mod text;
mod http;

use std::{path::Path};
use clap::Parser;
use std::path::PathBuf;
use crate::CmdExector;

pub use self::{
    csv::CsvOpts,
    genpass::GenPassOpts,
    http::HttpSubCommand,
    text::{TextSubCommand, TextSignFormat},
};

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

    #[command(subcommand, about="Base64 encode/decode")]
    Base64(Base64SubCommand),

    #[command(subcommand, about="Text sign/verify")]
    Text(TextSubCommand),

    #[command(subcommand, about="HTTP server")]
    Http(HttpSubCommand),
}

impl CmdExector for SubCommand {
    async fn execute(self) -> anyhow::Result<()> {
        match self {
            SubCommand::Csv(opts) => opts.execute().await,
            SubCommand::GenPass(opts) => opts.execute().await,
            SubCommand::Base64(cmd) => cmd.execute().await,
            SubCommand::Text(cmd) => cmd.execute().await,  
            SubCommand::Http(cmd) => cmd.execute().await,
        }
    }
}


fn verify_file(filename: &str) -> Result<String, &'static str> {
  if filename == "-" || Path::new(filename).exists() {
      Ok(filename.into())
  } else {
      Err("File does not exist")
  }
}


fn verify_path(path: &str) -> Result<PathBuf, &'static str> {
    let p = Path::new(path);
    if p.exists()  && p.is_dir() {
        Ok(path.into())
    } else {
        Err("Path does not exist or is not a directory")
    }
}

//单元测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_input_file() {
        assert_eq!(verify_file("-"), Ok("-".into()));
        assert_eq!(verify_file("*"), Err("File does not exist"));
        assert_eq!(verify_file("Cargo.toml"), Ok("Cargo.toml".into()));
        assert_eq!(verify_file("not-exist"), Err("File does not exist"));
    }
}