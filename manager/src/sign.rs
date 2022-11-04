use serde::{Deserialize, Serialize};
use crate::AmqpPool;

#[derive(Debug, Serialize, Deserialize)]
struct SignParams {
  user_id: String,
  room_id: String,
  public_key: String,
  data: String,
  participants_indexes: Vec<u16>
}

pub async fn sign_approve(params: serde_json::Value, pool: AmqpPool) -> anyhow::Result<()> {
  let params: SignParams = serde_json::from_value(params)?;

  Ok(())
}
