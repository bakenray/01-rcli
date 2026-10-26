use clap::Parser;
use rcli::{ process_csv, Opts, SubCommand, process_genpass };

fn main() -> anyhow::Result<()> {
  let opts: Opts = Opts::parse();

  match opts.cmd {
    SubCommand::Csv(csv_opts) => {
      let output = if let Some(output) = csv_opts.output {
        output
      } else {
        format!("output.{}", csv_opts.format)
      };
      process_csv(&csv_opts.input, output, csv_opts.format)?;
    }
    SubCommand::GenPass(gp_opts) => {
      println!("Generate passwor: {:?}", gp_opts);
      process_genpass(gp_opts.length, gp_opts.upper, gp_opts.lower, gp_opts.number, gp_opts.symbol)?;
    }
  }
  Ok(())
}