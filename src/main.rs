use clap::Parser;
// CmdExecutor 必须在作用域内，opts.cmd.execute() 这个 trait 方法才能被调用
use rcli::{CmdExecutor, Opts};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let opts = Opts::parse();

    opts.cmd.execute().await?;

    Ok(())
}
