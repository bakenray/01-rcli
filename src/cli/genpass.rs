use clap::Parser;
use crate::CmdExector;
use zxcvbn::zxcvbn;

#[derive(Debug, Parser)]
pub struct GenPassOpts {
    #[arg(short, long, default_value_t = 16)]
    pub length: u8,

    #[arg(long, default_value_t = false)]
    pub upper: bool,

    #[arg(long, default_value_t = false)]
    pub lower: bool,

    #[arg(long, default_value_t = true)]
    pub number: bool,

    #[arg(long, default_value_t = true)]
    pub symbol: bool,
}

impl CmdExector for GenPassOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let ret = crate::process_genpass(
            self.length,
            self.upper,
            self.lower,
            self.number,
            self.symbol
        )?;
        println!("{}", ret);
        
        let estimate = zxcvbn(&ret, &[]);
        eprintln!("Password strength: {}", estimate.score());
        Ok(())
    }
}