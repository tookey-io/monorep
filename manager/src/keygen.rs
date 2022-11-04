use std::env;
use std::ops::Deref;
use anyhow::{anyhow, Context};
use bb8_lapin::lapin::BasicProperties;
use bb8_lapin::lapin::options::BasicPublishOptions;
use curv::elliptic::curves::Secp256k1;
use round_based::{AsyncProtocol, Msg};
use tss::ecdsa::state_machine::keygen::{Keygen, ProtocolMessage};
use tss::ecdsa::state_machine::keygen::LocalKey;
use futures::{SinkExt, StreamExt};
use futures::channel::mpsc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use crate::AmqpPool;
use crate::util::join_computation;
use hex::ToHex;

pub fn relay_address() -> surf::Url {
  env::var("RELAY_ADDRESS").unwrap_or_else(|_| "http://localhost:8000".to_owned()).parse().unwrap()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KeygenParams {
  user_id: String,
  room_id: String,
  participant_index: u16,
  participants_count: u16,
  participants_threshold: u16,
}

pub async fn action_keygen_join(params: serde_json::Value, pool: AmqpPool) -> anyhow::Result<()> {
  let params: KeygenParams = serde_json::from_value(params)?;

  let (_i, incoming, outgoing) = join_computation(relay_address(), params.room_id.as_str())
    .await
    .context("join computation")?;

  // Collect senders and send progress statuses
  let (mut sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<u16>();
  let incoming = incoming.map(move |item: anyhow::Result<Msg<ProtocolMessage>>| {
    if let Ok(i) = &item {
      sender.send(i.sender).unwrap()
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

        let result = send_keygen_status(pool_clone.clone(), params_clone.room_id.clone(), params_clone.user_id.clone(), active.len() == params_clone.participants_count as usize, active.clone(), "".to_owned()).await;
        if let Err(err) = result { log::error!("Failed to send keygen status: {:?}", err) }
      }
    }
  });

  // Proceed with protocol
  let incoming = incoming.fuse();
  tokio::pin!(incoming);
  tokio::pin!(outgoing);

  let keygen = Keygen::new(params.participant_index, params.participants_threshold, params.participants_count)?;
  let output: LocalKey<Secp256k1> = AsyncProtocol::new(keygen, incoming, outgoing)
    .run()
    .await
    .map_err(|e| anyhow!("protocol execution terminated with error: {}", e))?;

  let public_key: String = output.public_key().to_bytes(true).deref().encode_hex();
  let output = serde_json::to_string(&output)?;
  log::debug!("Generated key for {}, public key: {}", params.user_id, public_key);

  // TODO: Write key to Vault
  send_keygen_status(pool.clone(), params.room_id, params.user_id, true, Vec::new(), public_key).await?;

  Ok(())
}

async fn send_keygen_status(pool: AmqpPool, room_id: String, user_id: String, finished: bool, active_indexes: Vec<u16>, public_key: String) -> anyhow::Result<()> {
  let conn = pool.get().await?;

  let channel = conn.create_channel().await?;

  let msg = json!({ "action": "keygen_status", "room_id": room_id, "user_id": user_id, "finished": finished, "active_indexes": active_indexes, "public_key": public_key });
  let msg = serde_json::to_string(&msg)?;

  // log::debug!("Rabbit: Sending to backend {}", msg);

  channel.basic_publish("amq.topic", "backend", BasicPublishOptions::default(), msg.as_bytes(), BasicProperties::default()).await?;

  Ok(())
}
