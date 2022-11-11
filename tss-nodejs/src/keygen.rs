use napi_derive::napi;
use std::time::Duration;
use tss::ecdsa::state_machine::keygen::{Keygen, LocalKey};
use tss::join::join_computation;

use anyhow::{anyhow, Context};
use curv::elliptic::curves::Secp256k1;
use futures::StreamExt;
use round_based::AsyncProtocol;

#[napi(object)]
pub struct KeygenParams {
    pub room_id: String,
    pub participant_index: u16,
    pub participants_count: u16,
    pub participants_threshold: u16,
    pub relay_address: String,
    pub timeout_seconds: u16,
}

#[napi(object)]
pub struct KeygenResult {
    pub key: Option<String>,
    pub error: Option<String>,
}

#[napi]
#[allow(dead_code)]
pub async fn keygen(params: KeygenParams) -> KeygenResult {
    match internal_keygen(params).await {
        Ok(val) => KeygenResult {
            key: Some(serde_json::to_string(&val).unwrap()),
            error: None,
        },
        Err(err) => KeygenResult {
            key: None,
            error: Some(format!("{:?}", err)),
        },
    }
}

async fn internal_keygen(params: KeygenParams) -> anyhow::Result<LocalKey<Secp256k1>> {
    let (_i, incoming, outgoing) = join_computation(params.relay_address.parse()?, params.room_id.as_str())
        .await
        .context("join computation")?;

    let incoming = incoming.fuse();
    napi::tokio::pin!(incoming);
    napi::tokio::pin!(outgoing);

    let keygen = Keygen::new(
        params.participant_index,
        params.participants_threshold,
        params.participants_count,
    )?;

    let mut protocol = AsyncProtocol::new(keygen, incoming, outgoing);
    match napi::tokio::time::timeout(Duration::from_secs(params.timeout_seconds as u64), protocol.run()).await {
        Ok(result) => match result {
            Ok(val) => Ok(val),
            Err(err) => Err(anyhow!("{:?}", err)),
        },
        Err(_) => Err(anyhow!("Timed out")),
    }
}
