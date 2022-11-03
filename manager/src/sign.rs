use serde_json::{Map, Value};

pub async fn sign_start(request: Map<String, Value>, pool: AmqpPool) -> anyhow::Result<()> {
  Ok(())
}

pub async fn sign_approve(request: Map<String, Value>, pool: AmqpPool) -> anyhow::Result<()> {
  Ok(())
}
