use clap::Parser;
use rcli::{ 
  process_csv,
  process_genpass,
  process_encode,
  process_decode,
  Opts, 
  SubCommand,
  Base64SubCommand,
};

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
      process_genpass(
        gp_opts.length, 
        gp_opts.upper, 
        gp_opts.lower, 
        gp_opts.number, 
        gp_opts.symbol
      )?;
    }
    SubCommand::Base64(subcmd) => match subcmd {
      Base64SubCommand::Encode(e_opts) => {
        process_encode(&e_opts.input, e_opts.format)?;
      }
      Base64SubCommand::Decode(d_opts) => {
        process_decode(&d_opts.input, d_opts.format)?;
      }
    }
  }
  Ok(())
}