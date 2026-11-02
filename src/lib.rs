// 注意：trait 必须声明在 `mod cli;` 之前。
// enum_dispatch 在处理 `#[enum_dispatch]`（trait 侧）时，会把“已经缓存但尚未链接”的
// enum 的 dispatch 代码生成到 trait 所在的位置；若顺序反过来，生成的代码会落在 crate 根，
// 那里看不到 cli 模块里的 CsvOpts / Base64EncodeOpts 等类型，直接编译失败。
use enum_dispatch::enum_dispatch;

#[enum_dispatch]
#[allow(async_fn_in_trait)]
pub trait CmdExector {
    async fn execute(self) -> anyhow::Result<()>;
}

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
