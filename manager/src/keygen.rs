use std::ops::Deref;
use std::time::Duration;

use anyhow::{anyhow, Context};
use curv::elliptic::curves::Secp256k1;
use futures::StreamExt;
use hex::ToHex;
use round_based::{AsyncProtocol, Msg};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::sleep;

use tss::ecdsa::state_machine::keygen::LocalKey;
use tss::ecdsa::state_machine::keygen::{Keygen, ProtocolMessage};

use crate::amqp::amqp_send_notification;
use crate::config::Config;
use crate::secrets::store_key;
use crate::util::join_computation;
use crate::AmqpPool;

pub enum TaskStatus {
  Created,
  Started,
  Finished,
  Error,
  Timeout,
}

impl TaskStatus {
  pub fn to_string(&self) -> &'static str {
    match self {
      TaskStatus::Created => "created",
      TaskStatus::Started => "started",
      TaskStatus::Finished => "finished",
      TaskStatus::Error => "error",
      TaskStatus::Timeout => "timeout",
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KeygenParams {
  user_id: String,
  key_id: String,
  room_id: String,
  participant_index: u16,
  participants_count: u16,
  participants_threshold: u16,

  #[serde(default = "Config::relay_address")]
  relay_address: String,

  #[serde(default = "Config::default_timeout_seconds")]
  timeout_seconds: u64,
}

pub async fn action_keygen_join(params: serde_json::Value, pool: AmqpPool) -> anyhow::Result<()> {
  let params: KeygenParams = serde_json::from_value(params)?;

  let (_i, incoming, outgoing) = join_computation(params.relay_address.parse()?, params.room_id.as_str())
    .await
    .context("join computation")?;

  // Notify about keygen start if first user
  if params.participant_index == 1 {
    send_keygen_status(
      pool.clone(),
      params.room_id.clone(),
      TaskStatus::Created,
      Some(vec![params.participant_index]),
      None,
    )
    .await?;
  }

  // Collect senders and send progress statuses
  let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<u16>();
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
          params_clone.room_id.clone(),
          TaskStatus::Started,
          Some(active.clone()),
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

  let mut protocol = AsyncProtocol::new(keygen, incoming, outgoing);
  let output: LocalKey<Secp256k1> =
    match tokio::time::timeout(Duration::from_secs(params.timeout_seconds), protocol.run()).await {
      Ok(result) => match result {
        Ok(output) => output,
        Err(err) => {
          send_keygen_status(pool.clone(), params.room_id, TaskStatus::Error, None, None).await?;
          return Err(anyhow!("protocol execution terminated with error: {}", err));
        }
      },
      Err(_) => {
        send_keygen_status(pool.clone(), params.room_id, TaskStatus::Timeout, None, None).await?;
        return Err(anyhow::anyhow!("Timed out"));
      }
    };

  let public_key: String = output.public_key().to_bytes(true).deref().encode_hex();

  store_key(&params.user_id, &params.key_id, &output).await?;

  send_keygen_status(
    pool.clone(),
    params.room_id,
    TaskStatus::Finished,
    Some((1..=params.participants_count).into_iter().collect()),
    Some(public_key),
  )
  .await?;

  // Wait for outgoing messages to be flushed
  sleep(Duration::from_millis(1000)).await;

  Ok(())
}

async fn send_keygen_status(
  pool: AmqpPool,
  room_id: String,
  status: TaskStatus,
  active_indexes: Option<Vec<u16>>,
  public_key: Option<String>,
) -> anyhow::Result<()> {
  let msg = json!({
    "action": "keygen_status",
    "room_id": room_id,
    "status": status.to_string(),
    "active_indexes": active_indexes,
    "public_key": public_key
  });
  let msg = serde_json::to_string(&msg)?;

  amqp_send_notification(pool, msg).await?;

  Ok(())
}
