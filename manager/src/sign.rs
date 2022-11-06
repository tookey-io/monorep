use crate::util::join_computation;
use crate::{fetch_key, AmqpPool};
use anyhow::{anyhow, Context};
use bb8_lapin::lapin::options::BasicPublishOptions;
use bb8_lapin::lapin::BasicProperties;
use curv::arithmetic::Converter;
use curv::elliptic::curves::Secp256k1;
use curv::BigInt;
use ethereum_types::H256;
use futures::SinkExt;
use futures::StreamExt;
use round_based::{AsyncProtocol, Msg};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use tss::ecdsa::state_machine::keygen::Keygen;
use tss::ecdsa::state_machine::keygen::LocalKey;
use tss::ecdsa::state_machine::sign::{OfflineProtocolMessage, PartialSignature};
use tss_ceremonies::ecdsa;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SignParams {
  user_id: String,
  key_id: String,
  room_id: String,
  relay_address: String,
  data: String,
  participants_indexes: Vec<u16>,
}

pub async fn sign_approve(params: Value, pool: AmqpPool) -> anyhow::Result<()> {
  let params: SignParams = serde_json::from_value(params)?;

  let (_i, incoming, outgoing) = join_computation(params.relay_address.parse()?, params.room_id.as_str())
    .await
    .context("join computation")?;

  // Collect senders and send progress statuses
  let (mut sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<u16>();
  let incoming = incoming.map(move |item: anyhow::Result<Msg<Value>>| {
    if let Ok(i) = &item {
      // Ignore errors
      let _ = sender.send(i.sender);
    }

    item
  });

  let params_clone = params.clone();
  let pool_clone = pool.clone();
  tokio::spawn(async move {
    let mut active = Vec::new();
    active.push(1);

    while let Some(v) = receiver.recv().await {
      if !active.contains(&v) {
        active.push(v);

        let result = send_sign_status(
          pool_clone.clone(),
          params_clone.room_id.clone(),
          "started",
          active.clone(),
          None,
        )
        .await;
        if let Err(err) = result {
          log::error!("Failed to send keygen status: {:?}", err)
        }
      }
    }
  });

  // Proceed with protocol
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

  let key = fetch_key(&params.user_id, &params.key_id).await?;
  let hash = BigInt::from_bytes(H256::from_str(params.data.as_str()).context("hash read")?.as_bytes());
  let signature = match tss_ceremonies::ecdsa::sign(
    outgoing_sender,
    incoming,
    key,
    params.participants_indexes.clone(),
    hash,
  )
  .await
  {
    Ok(sign) => sign,
    Err(err) => {
      send_sign_status(pool.clone(), params.room_id, "error", Vec::new(), None).await?;

      return Err(err);
    }
  };

  let signature = serde_json::to_string(&signature).context("serialize signature")?;

  send_sign_status(
    pool.clone(),
    params.room_id,
    "finished",
    params.participants_indexes,
    Some(signature),
  )
  .await?;

  Ok(())
}

async fn send_sign_status(
  pool: AmqpPool,
  room_id: String,
  status: &'static str,
  active_indexes: Vec<u16>,
  result: Option<String>,
) -> anyhow::Result<()> {
  let conn = pool.get().await?;

  let channel = conn.create_channel().await?;

  let msg = json!({ "action": "sign_status", "room_id": room_id, "status": status, "active_indexes": active_indexes, "result": result });
  let msg = serde_json::to_string(&msg)?;

  log::trace!("Rabbit: Sending to backend {}", msg);

  // TODO: Replace with fanout routing
  channel
    .basic_publish(
      "amq.topic",
      "backend",
      BasicPublishOptions::default(),
      msg.as_bytes(),
      BasicProperties::default(),
    )
    .await?;

  Ok(())
}
