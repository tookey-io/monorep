use std::env;
use anyhow::{anyhow, Context};
use round_based::AsyncProtocol;
use serde_json::{Map, Value};
use tss::ecdsa::state_machine::keygen::Keygen;
use crate::AmqpPool;
use crate::util::join_computation;

pub fn relay_address() -> surf::Url {
  env::var("RELAY_ADDRESS").unwrap_or_else(|_| "http://localhost:8000".to_owned()).parse().unwrap()
}

pub async fn action_keygen_start(request: Map<String, Value>, pool: AmqpPool) -> anyhow::Result<()> {
  let room_id = request.get("room_id").context("room_id is required for Keygen_start request")?.as_str().context("room_id should be a string")?.to_owned();
  let participants_number = request.get("participants_number").context("participants_number is required for Keygen_start request")?.as_u64().context("participants_number should be a u64")? as u16;
  let required_participants_number = request.get("required_participants_number").context("required_participants_number is required for Keygen_start request")?.as_u64().context("required_participants_number should be a u64")? as u16;

  // Create new record in DB

  tokio::spawn(async move {
    if let Err(err) = join_keygen(room_id, participants_number, required_participants_number, 0).await {
      log::error!("Failed to join keygen: {:?}", err);
    }
  });

  Ok(())
}

pub async fn action_keygen_join(request: Map<String, Value>, pool: AmqpPool) -> anyhow::Result<()> {
  let room_id = request.get("room_id").context("room_id is required for Keygen_start request")?.as_str().context("room_id should be a string")?.to_owned();
  let index = request.get("participant_number").context("participant_number is required for Keygen_start request")?.as_u64().context("participants_number should be a u64")? as u16;

  // Fetch from DB
  let participants_number = 0 as u16;
  let required_participants_number = 0 as u16;

  tokio::spawn(async move {
    if let Err(err) = join_keygen(room_id, participants_number, required_participants_number, index).await {
      log::error!("Failed to join keygen: {:?}", err);
    }
  });

  Ok(())
}

// TODO: Replace arguments with Keygen DB object
async fn join_keygen(room_id: String, participants_number: u16, required_participants_number: u16, index: u16) -> anyhow::Result<()> {
  let (_i, incoming, outgoing) = join_computation(relay_address(), &room_id)
    .await
    .context("join computation")?;

  let incoming = incoming.fuse();

  tokio::pin!(incoming);
  tokio::pin!(outgoing);

  let keygen = Keygen::new(index, required_participants_number, participants_number)?;
  let output = AsyncProtocol::new(keygen, incoming, outgoing)
    .run()
    .await
    .map_err(|e| anyhow!("protocol execution terminated with error: {}", e))?;

  // Write key to Vault

  // Send keygen status to Pool
  Ok(())
}
