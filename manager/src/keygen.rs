use crate::secrets::store_key;
use crate::util::join_computation;
use crate::AmqpPool;
use anyhow::{anyhow, Context, Error};
use bb8_lapin::lapin::options::BasicPublishOptions;
use bb8_lapin::lapin::BasicProperties;
use curv::elliptic::curves::Secp256k1;
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use hex::ToHex;
use round_based::{AsyncProtocol, Msg};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::ops::Deref;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tss::ecdsa::state_machine::keygen::LocalKey;
use tss::ecdsa::state_machine::keygen::{Keygen, ProtocolMessage};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KeygenParams {
  user_id: String,
  key_id: String,
  room_id: String,
  relay_address: String,
  participant_index: u16,
  participants_count: u16,
  participants_threshold: u16,
}

pub async fn action_keygen_join(params: serde_json::Value, pool: AmqpPool) -> anyhow::Result<()> {
  let params: KeygenParams = serde_json::from_value(params)?;

  let (_i, incoming, outgoing) = join_computation(params.relay_address.parse()?, params.room_id.as_str())
    .await
    .context("join computation")?;

  // Collect senders and send progress statuses
  let (mut sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<u16>();
  let incoming = incoming.map(move |item: anyhow::Result<Msg<ProtocolMessage>>| {
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

        let result = send_keygen_status(
          pool_clone.clone(),
          params_clone.user_id.clone(),
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
  let incoming = incoming.fuse();
  tokio::pin!(incoming);
  tokio::pin!(outgoing);

  let keygen = Keygen::new(
    params.participant_index,
    params.participants_threshold,
    params.participants_count,
  )?;

  let output: LocalKey<Secp256k1> = match AsyncProtocol::new(keygen, incoming, outgoing)
    .run()
    .await
    .map_err(|e| anyhow!("protocol execution terminated with error: {}", e))
  {
    Ok(output) => output,
    Err(err) => {
      send_keygen_status(pool.clone(), params.user_id, params.room_id, "error", Vec::new(), None).await?;
      return Err(err);
    }
  };

  let public_key: String = output.public_key().to_bytes(true).deref().encode_hex();

  store_key(&params.user_id, &params.key_id, &output).await?;

  send_keygen_status(
    pool.clone(),
    params.user_id,
    params.room_id,
    "finished",
    (1..=params.participants_count).into_iter().collect(),
    Some(public_key),
  )
  .await?;

  Ok(())
}

async fn send_keygen_status(
  pool: AmqpPool,
  user_id: String,
  room_id: String,
  status: &'static str,
  active_indexes: Vec<u16>,
  public_key: Option<String>,
) -> anyhow::Result<()> {
  let conn = pool.get().await?;

  let channel = conn.create_channel().await?;

  let msg = json!({ "action": "keygen_status", "user_id": user_id, "room_id": room_id, "status": status, "active_indexes": active_indexes, "public_key": public_key });
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
