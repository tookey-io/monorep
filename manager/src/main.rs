use std::env;
use std::str::FromStr;

use anyhow::Context;
use bb8_lapin::lapin::options::BasicAckOptions;
use futures::StreamExt;
use serde_json::Value;

use crate::amqp::{amqp_init, amqp_subscribe, AmqpPool};
use crate::config::Config;
use crate::keygen::action_keygen_join;
use crate::sign::sign_approve;

mod amqp;
mod config;
mod keygen;
mod secrets;
mod sign;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  pretty_env_logger::formatted_timed_builder()
    .filter(
      Some(&env!("CARGO_PKG_NAME").replace('-', "_")),
      log::LevelFilter::from_str(&env::var("RUST_LOG").unwrap_or_else(|_| String::from("info")))?,
    )
    .init();

  let pool = amqp_init().await?;
  let mut consumer = amqp_subscribe().await?;

  // TODO: Nack requests if already handling > TASKS_LIMIT tasks (increment on spawn with AtomicU32, decrement on task finish)
  while let Some(delivery) = consumer.next().await {
    let delivery = delivery.expect("error in consumer");

    if let Err(err) = handle(delivery.data.clone(), pool.clone()).await {
      log::error!("Failed to process action: {:?}", err)
    }

    delivery.ack(BasicAckOptions::default()).await.expect("Ack failed");
  }

  Ok(())
}

async fn handle(data: Vec<u8>, pool: AmqpPool) -> anyhow::Result<()> {
  let data = String::from_utf8(data)?;
  log::trace!("AMQP: Received {}", data);

  let data: Value = serde_json::from_str(data.as_str())?;
  let action = data
    .as_object()
    .context("Message is not an object")?
    .get("action")
    .context("Message doesn't include action key")?
    .as_str()
    .context("Action isn't a string")?
    .to_owned();

  tokio::spawn(async move {
    let result = match action.as_str() {
      "keygen_join" => action_keygen_join(data.clone(), pool).await,
      "sign_approve" => sign_approve(data.clone(), pool).await,
      action => {
        log::error!("Unknown action: {}", action);
        Ok(())
      }
    };

    if let Err(err) = result {
      log::error!("Failed to execute action {}: {:?} (data: {})", action, err, data);
    }
  });

  Ok(())
}
