use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::amqp::{amqp_send_notification, AmqpPool};
use crate::config::Config;
use crate::keygen::TaskStatus;
use crate::secrets::fetch_key;


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
  log::trace!("sign_approve, params: {:?}", params);
  let params: SignParams = serde_json::from_value(params)?;

  let key = fetch_key(&params.user_id, &params.key_id).await?;

  send_sign_status(
    pool.clone(),
    params.room_id.clone(),
    TaskStatus::Created,
    Some(vec![key.i]),
    None,
  )
  .await?;

  let result = tookey_libtss::sign::sign(tookey_libtss::sign::SignParams {
    room_id: params.room_id.clone(),
    key: serde_json::to_string(&key)?,
    data: params.data,
    participants_indexes: params.participants_indexes.clone(),
    relay_address: params.relay_address,
    timeout_seconds: params.timeout_seconds as u16,
  })
  .await;

  let signature = match result {
    tookey_libtss::sign::SignResult {
      result: Some(result),
      error: None,
    } => Ok(result),
    tookey_libtss::sign::SignResult {
      result: None,
      error: Some(err),
    } => Err(anyhow!(err)),
    _ => Err(anyhow!("unreachable")),
  }?;

  send_sign_status(
    pool.clone(),
    params.room_id,
    TaskStatus::Finished,
    Some(params.participants_indexes),
    Some(signature),
  )
  .await?;

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
