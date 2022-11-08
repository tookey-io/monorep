use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use curv::arithmetic::Converter;
use curv::BigInt;
use ethereum_types::H256;
use futures::SinkExt;
use futures::StreamExt;
use round_based::Msg;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;

use tss::ecdsa::state_machine::sign::{OfflineProtocolMessage, PartialSignature};
use tss_ceremonies::ecdsa;

use crate::amqp::{amqp_send_notification, AmqpPool};
use crate::config::Config;
use crate::keygen::TaskStatus;
use crate::secrets::fetch_key;
use crate::util::join_computation;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SignParams {
  user_id: String,
  key_id: String,
  room_id: String,
  data: String,
  participants_indexes: Vec<u16>,

  #[serde(default = "Config::relay_address")]
  relay_address: String,

  #[serde(default = "Config::default_timeout_seconds")]
  timeout_seconds: u64,
}

pub async fn sign_approve(params: Value, pool: AmqpPool) -> anyhow::Result<()> {
  let params: SignParams = serde_json::from_value(params)?;

  let (_i, incoming, outgoing) = join_computation(params.relay_address.parse()?, params.room_id.as_str())
    .await
    .context("join computation")?;

  let key = fetch_key(&params.user_id, &params.key_id).await?;

  // Notify about sign start if first user
  if key.i == 1 {
    send_sign_status(
      pool.clone(),
      params.room_id.clone(),
      TaskStatus::Created,
      Some(vec![key.i]),
      None,
    )
    .await?;
  }

  // Collect senders and send progress statuses
  let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<u16>();
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
        msg.map_body(|body| serde_json::to_value(body).unwrap_or_else(|_| Value::Null))
      }
      ecdsa::Messages::PartialSignature(msg) => {
        msg.map_body(|body| serde_json::to_value(body).unwrap_or_else(|_| Value::Null))
      }
    };

    outgoing.send(packet).await.context("sending")?;

    Ok::<_, anyhow::Error>(outgoing)
  });

  let hash = BigInt::from_bytes(H256::from_str(params.data.as_str()).context("hash read")?.as_bytes());

  let signature_future = tss_ceremonies::ecdsa::sign(
    outgoing_sender,
    incoming,
    key,
    params.participants_indexes.clone(),
    hash,
  );
  let signature = match tokio::time::timeout(Duration::from_secs(params.timeout_seconds), signature_future).await {
    Ok(result) => match result {
      Ok(sign) => sign,
      Err(err) => {
        send_sign_status(pool.clone(), params.room_id, TaskStatus::Error, None, None).await?;
        return Err(err);
      }
    },
    Err(_) => {
      send_sign_status(pool.clone(), params.room_id, TaskStatus::Timeout, None, None).await?;
      return Err(anyhow::anyhow!("Timed out"));
    }
  };

  let signature = serde_json::to_string(&signature).context("serialize signature")?;

  send_sign_status(
    pool.clone(),
    params.room_id,
    TaskStatus::Finished,
    Some(params.participants_indexes),
    Some(signature),
  )
  .await?;

  // Wait for outgoing messages to be flushed
  sleep(Duration::from_millis(1000)).await;

  Ok(())
}

async fn send_sign_status(
  pool: AmqpPool,
  room_id: String,
  status: TaskStatus,
  active_indexes: Option<Vec<u16>>,
  result: Option<String>,
) -> anyhow::Result<()> {
  let msg = json!({
    "action": "sign_status",
    "room_id": room_id,
    "status": status.to_string(),
    "active_indexes": active_indexes,
    "result": result
  });
  let msg = serde_json::to_string(&msg)?;

  amqp_send_notification(pool, msg).await?;

  Ok(())
}
