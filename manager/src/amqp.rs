use bb8::{ManageConnection, Pool};
use bb8_lapin::lapin::options::{
    BasicConsumeOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use bb8_lapin::lapin::types::FieldTable;
use bb8_lapin::lapin::{BasicProperties, ConnectionProperties, Consumer, ExchangeKind};
use bb8_lapin::LapinConnectionManager;

use crate::Config;

pub type AmqpPool = Pool<LapinConnectionManager>;

pub async fn amqp_init() -> anyhow::Result<AmqpPool> {
    let manager = LapinConnectionManager::new(&Config::amqp_address(), ConnectionProperties::default());
    let pool = Pool::builder().max_size(10).build(manager).await?;

    Ok(pool)
}

pub async fn amqp_subscribe() -> anyhow::Result<Consumer> {
    let manager = LapinConnectionManager::new(&Config::amqp_address(), ConnectionProperties::default());
    let conn = manager.connect().await?;
    let channel = conn.create_channel().await?;

    let amqp_exchange = Config::amqp_listen_exchange();
    let amqp_queue = Config::amqp_listen_queue();

    if amqp_exchange != "amq.topic" {
        channel
            .exchange_declare(
                amqp_exchange.as_str(),
                ExchangeKind::Topic,
                exchange_options(),
                FieldTable::default(),
            )
            .await?;
    }

    channel
        .queue_declare(amqp_queue.as_str(), queue_options(), FieldTable::default())
        .await?;
    channel
        .queue_bind(
            amqp_queue.as_str(),
            amqp_exchange.as_str(),
            amqp_queue.as_str(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;
    let consumer = channel
        .basic_consume(
            amqp_queue.as_str(),
            "",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    log::info!("AMQP: Subscribed to {}#{}", amqp_exchange, amqp_queue);

    Ok(consumer)
}

pub async fn amqp_send_notification(pool: AmqpPool, msg: String) -> anyhow::Result<()> {
    log::trace!("AMQP: Sending notification {}", msg);

    // TODO: Replace with fanout routing to support multiple receivers
    let conn = pool.get().await?;

    let channel = conn.create_channel().await?;
    channel
        .basic_publish(
            Config::amqp_notifications_exchange().as_str(),
            Config::amqp_notifications_queue().as_str(),
            BasicPublishOptions::default(),
            msg.as_bytes(),
            BasicProperties::default(),
        )
        .await?;

    Ok(())
}

fn queue_options() -> QueueDeclareOptions {
    let env = Config::env();

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
