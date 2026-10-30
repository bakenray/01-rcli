mod cli;
mod process;
mod utils;

pub use cli::{
    SubCommand, 
    Base64SubCommand,
    TextSubCommand,
    HttpSubCommand,
    Opts, 
    Base64Format, 
    TextSignFormat,
};
pub use process::*;
pub use utils::*;

#[allow(async_fn_in_trait)]
pub trait CmdExector {
    async fn execute(self) -> anyhow::Result<()>;
}