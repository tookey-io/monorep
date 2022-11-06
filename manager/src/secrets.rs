use curv::elliptic::curves::Secp256k1;
use vaultrs::client::{VaultClient, VaultClientSettingsBuilder};
use vaultrs::kv2;

use crate::Config;
use tss::ecdsa::state_machine::keygen::LocalKey;

pub async fn store_key(user_id: &str, key_id: &str, key: &LocalKey<Secp256k1>) -> anyhow::Result<()> {
  let client = build_client()?;

  kv2::set(&client, "secret", vec![user_id, key_id].join("/").as_str(), key).await?;

  Ok(())
}

pub async fn fetch_key(user_id: &str, key_id: &str) -> anyhow::Result<LocalKey<Secp256k1>> {
  let client = build_client()?;

  Ok(kv2::read(&client, "secret", vec![user_id, key_id].join("/").as_str()).await?)
}

fn build_client() -> anyhow::Result<VaultClient> {
  let client = VaultClient::new(
    VaultClientSettingsBuilder::default()
      .address(Config::vault_address())
      .token(Config::vault_token())
      .build()?,
  )?;

  Ok(client)
}
