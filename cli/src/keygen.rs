use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use std::path::PathBuf;
use structopt::StructOpt;
use tookey_libtss::ecdsa::state_machine::keygen::Keygen;
use tookey_libtss::join::join_computation;
use tookey_libtss::round_based::AsyncProtocol;

#[derive(Debug, StructOpt)]
struct Cli {
  #[structopt(short, long, default_value = "http://localhost:8000/")]
  address: String,
  #[structopt(short, long, default_value = "default-keygen")]
  room: String,
  #[structopt(short, long)]
  output: PathBuf,

  #[structopt(short, long)]
  index: u16,
  #[structopt(short, long)]
  threshold: u16,
  #[structopt(short, long)]
  number_of_parties: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
  let args: Cli = Cli::from_args();
  let mut output_file = tokio::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .open(args.output)
    .await
    .context("cannot create output file")?;

  let (_i, incoming, outgoing) = join_computation(args.address, &args.room)
    .await
    .context("join computation")?;

  let incoming = incoming.fuse();
  tokio::pin!(incoming);
  tokio::pin!(outgoing);

  let keygen = Keygen::new(args.index, args.threshold, args.number_of_parties)?;
  let output = AsyncProtocol::new(keygen, incoming, outgoing)
    .run()
    .await
    .map_err(|e| anyhow!("protocol execution terminated with error: {:?}", e))?;

  let output = serde_json::to_vec_pretty(&output).context("serialize output")?;
  tokio::io::copy(&mut output.as_slice(), &mut output_file)
    .await
    .context("save output to file")?;

  Ok(())
}
