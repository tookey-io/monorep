use std::env;

pub struct Config {}

impl Config {
  pub fn default_timeout_seconds() -> u64 {
    120
  }

  pub fn env() -> String {
    env::var("APP_ENV").unwrap_or_else(|_| "production".to_owned())
  }

  pub fn amqp_address() -> String {
    env::var("AMQP_ADDRESS").unwrap_or_else(|_| "amqp://guest:guest@127.0.0.1:5672//".to_owned())
  }

  pub fn amqp_listen_exchange() -> String {
    env::var("AMQP_LISTEN_EXCHANGE").unwrap_or_else(|_| "amq.topic".to_owned())
  }

  pub fn amqp_listen_queue() -> String {
    env::var("AMQP_LISTEN_QUEUE").unwrap_or_else(|_| "manager".to_owned())
  }

  pub fn amqp_notifications_exchange() -> String {
    env::var("AMQP_NOTIFICATIONS_EXCHANGE").unwrap_or_else(|_| "amq.topic".to_owned())
  }

  pub fn amqp_notifications_queue() -> String {
    env::var("AMQP_NOTIFICATIONS_QUEUE").unwrap_or_else(|_| "backend".to_owned())
  }

  pub fn vault_address() -> String {
    env::var("VAULT_ADDRESS").unwrap_or_else(|_| "http://127.0.0.1:8200".to_owned())
  }

  pub fn vault_token() -> String {
    env::var("VAULT_TOKEN").unwrap_or_else(|_| "password".to_owned())
  }
}
