mod keygen;
mod sign;
mod util;

use std::env;
use std::str::FromStr;
use anyhow::Context;
use bb8::{ManageConnection, Pool};
use bb8_lapin::lapin::{ConnectionProperties, ExchangeKind};
use bb8_lapin::lapin::options::{BasicAckOptions, BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions};
use bb8_lapin::lapin::types::FieldTable;
use bb8_lapin::LapinConnectionManager;
use futures::StreamExt;
use serde_json::Value;
use crate::keygen::{action_keygen_join};
use crate::sign::{sign_approve};

pub type AmqpPool = Pool<LapinConnectionManager>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  pretty_env_logger::formatted_timed_builder()
    .filter(
      Some(&env!("CARGO_PKG_NAME").replace('-', "_")),
      log::LevelFilter::from_str(&env::var("RUST_LOG").unwrap_or_else(|_| String::from("debug")))?,
    )
    .init();

  let manager = LapinConnectionManager::new("amqp://guest:guest@127.0.0.1:5672//", ConnectionProperties::default());
  let conn = manager.connect().await?;
  let pool = Pool::builder()
    .max_size(10)
    .build(manager)
    .await?;

  let channel = conn.create_channel().await?;

  let rabbit_exchange = env::var("AMQP_EXCHANGE").unwrap_or(String::from("amq.topic"));
  let rabbit_queue = env::var("AMQP_QUEUE").unwrap_or(String::from("manager"));

  if rabbit_exchange != "amq.topic" {
    channel.exchange_declare(rabbit_exchange.as_str(), ExchangeKind::Topic, exchange_options(), FieldTable::default()).await?;
  }

  channel.queue_declare(rabbit_queue.as_str(), queue_options(), FieldTable::default()).await?;
  channel.queue_bind(rabbit_queue.as_str(), rabbit_exchange.as_str(), rabbit_queue.as_str(), QueueBindOptions::default(), FieldTable::default()).await?;

  let mut consumer = channel.basic_consume(rabbit_queue.as_str(), "", BasicConsumeOptions::default(), FieldTable::default()).await?;

  log::info!("Rabbit: Subscribed to {}.{}", rabbit_exchange, rabbit_queue);

  // TODO: Nack requests if already handling > TASKS_LIMIT tasks (count on spawn with AtomicU32)
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
  log::info!("Rabbit: Received {}", data);

  let data: Value = serde_json::from_str(data.as_str())?;
  let action = data.as_object().context("Message is not an object")?.get("action").context("Message doesn't include action key")?.as_str().context("Action isn't a string")?.to_owned();

  tokio::spawn(async move {
    let result = match action.as_str() {
      "keygen_join" => { action_keygen_join(data.clone(), pool).await }
      "sign_approve" => { sign_approve(data.clone(), pool).await }
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

fn queue_options() -> QueueDeclareOptions {
  let env = env::var("APP_ENV").unwrap_or(String::from("production"));

  if env == "test" {
    QueueDeclareOptions {
      durable: false,
      auto_delete: true,

      passive: false,
      exclusive: false,
      nowait: false,
    }
  } else {
    QueueDeclareOptions {
      durable: true,
      auto_delete: false,

      passive: false,
      exclusive: false,
      nowait: false,
    }
  }
}

fn exchange_options() -> ExchangeDeclareOptions {
  ExchangeDeclareOptions {
    passive: false,
    durable: true,
    auto_delete: false,
    internal: false,
    nowait: false,
  }
}
