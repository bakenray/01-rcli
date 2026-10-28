use std::fs;

use clap::Parser;
use rcli::{ 
  process_csv,
  process_genpass,
  process_encode,
  process_decode,
  process_text_sign,
  process_text_verify,
  process_text_generate,
  Opts,
  SubCommand,
  Base64SubCommand,
  TextSubCommand
};
use zxcvbn::zxcvbn;

fn main() -> anyhow::Result<()> {

  let opts = Opts::parse();

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
      // println!("Generate passwor: {:?}", gp_opts);
      let password = process_genpass(
        gp_opts.length, 
        gp_opts.upper, 
        gp_opts.lower, 
        gp_opts.number, 
        gp_opts.symbol
      )?;
      
      println!("{}", password);

      // output password strength in stderr
      eprintln!("Password strength: {}", zxcvbn(&password, &[]).score());
    }
    SubCommand::Base64(subcmd) => match subcmd {
      Base64SubCommand::Encode(e_opts) => {
        let encoded = process_encode(&e_opts.input, e_opts.format)?;
        println!("{}", encoded);

      }
      Base64SubCommand::Decode(d_opts) => {
        let decoded = process_decode(&d_opts.input, d_opts.format)?;
        let decoded = String::from_utf8(decoded)?;
        println!("{}", decoded);
      }
    }
    SubCommand::Text(subcmd) =>  match subcmd {
      TextSubCommand::Sign(opts) => {
        let signed = process_text_sign(&opts.input, &opts.key, opts.format)?;
        println!("{}", signed);
      }
      TextSubCommand::Verify(opts) => {
        let verified = process_text_verify(&opts.input, &opts.key,&opts.sig, opts.format)?;
        println!("{}", verified);
      }
      TextSubCommand::Generate(opts) => {
        let key = process_text_generate(opts.format)?;
        match opts.format {
          rcli::TextSignFormat::Blake3 => {
            let name = opts.output.join("blake3.txt");
            fs::write(name, &key[0])?;
          }
          rcli::TextSignFormat::Ed25519 => {
            let name = &opts.output;
            fs::write(name.join("ed25519.sk"), &key[0])?;
            fs::write(name.join("ed25519.pk"), &key[1])?;
          }
        }
      }

    }
  }
  Ok(())
}