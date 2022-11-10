use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use curv::arithmetic::Converter;
use curv::BigInt;
use ethereum_types::H256;
use futures::{SinkExt, StreamExt};
use structopt::StructOpt;

use curv::elliptic::curves::Secp256k1;

use tss::ecdsa::state_machine::keygen::LocalKey;
use tss::ecdsa::state_machine::sign::{OfflineProtocolMessage, PartialSignature};

use tss_ceremonies::ecdsa;
use tss_cli::join_computation;

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(short, long, default_value = "http://localhost:8000/")]
    address: surf::Url,
    #[structopt(short, long, default_value = "default-signing")]
    room: String,
    #[structopt(short, long)]
    local_share: PathBuf,
    #[structopt(short, long, use_delimiter(true))]
    parties: Vec<u16>,
    #[structopt(short, long)]
    hash: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Cli = Cli::from_args();
    let hash = BigInt::from_bytes(H256::from_str(args.hash.as_str()).context("hash read")?.as_bytes());
    let local_share = tokio::fs::read(args.local_share)
        .await
        .context("cannot read local share")?;
    let local_share: LocalKey<Secp256k1> = serde_json::from_slice(&local_share).context("parse local share")?;

    let (i, incoming, outgoing) = join_computation::<serde_json::Value>(args.address.clone(), &args.room)
        .await
        .context("join offline computation")?;

    let expected_id = args
        .parties
        .iter()
        .position(|p| *p == local_share.i)
        .map(|v| v + 1)
        .context("Not in party")? as u16;

    println!("My id is {} (expected {})", i, expected_id);

    if i != expected_id {
        return Err(anyhow!("Incorrect party id. Voting for group is not implemented yet"));
    }

    let incoming = incoming.filter_map(|msg| async move {
        match msg {
            Ok(msg) => {
                // TODO: fix coping
                let json = msg.body.clone();
                let possible_offline = serde_json::from_value::<OfflineProtocolMessage>(json.clone());
                let possible_partial = serde_json::from_value::<PartialSignature>(json);

                let wrapped_msg = match (possible_offline, possible_partial) {
                    (Ok(offline), _) => ecdsa::Messages::OfflineStage(msg.map_body(|_| offline)),
                    (_, Ok(partial)) => ecdsa::Messages::PartialSignature(msg.map_body(|_| partial)),
                    _ => unreachable!(),
                };

                Some(Ok(wrapped_msg))
            }
            Err(e) => Some(Err(e)),
        }
    });

    tokio::pin!(outgoing);

    let outgoing_sender = futures::sink::unfold(outgoing, |mut outgoing, msg| async move {
        let packet = match msg {
            ecdsa::Messages::OfflineStage(msg) => {
                msg.map_body(|body| serde_json::to_value(body).context("packet serialization").unwrap())
            }
            ecdsa::Messages::PartialSignature(msg) => {
                msg.map_body(|body| serde_json::to_value(body).context("packet serialization").unwrap())
            }
        };

        outgoing.send(packet).await.context("sending")?;

        Ok::<_, anyhow::Error>(outgoing)
    });

    let signature = tss_ceremonies::ecdsa::sign(outgoing_sender, incoming, local_share, args.parties, hash).await?;

    let signature = serde_json::to_string(&signature).context("serialize signature")?;
    println!("{}", signature);

    Ok(())
}
