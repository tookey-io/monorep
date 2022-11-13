use std::str::FromStr;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tookey_libtss::curv::arithmetic::Integer;
use tookey_libtss::curv::elliptic::curves::secp256_k1::{Secp256k1Point, Secp256k1Scalar};
use tookey_libtss::curv::elliptic::curves::{ECPoint, ECScalar, Point, Scalar, Secp256k1};
use tookey_libtss::curv::BigInt;
use web3::{
  ethabi::{ethereum_types::Signature, Address},
  signing::keccak256,
  types::{Recovery, RecoveryMessage, TransactionRequest, H256},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignatureRecid {
  pub r: Scalar<Secp256k1>,
  pub s: Scalar<Secp256k1>,
  pub recid: u64,
}

fn public_key_address(public_key: &Secp256k1Point) -> Address {
  let public_key = public_key.serialize_uncompressed();

  debug_assert_eq!(public_key[0], 0x04);
  let hash = keccak256(&public_key[1..]);

  Address::from_slice(&hash[12..])
}

/// Gets the address of a public key.
///
/// The public address is defined as the low 20 bytes of the keccak hash of
/// the public key. Note that the public key returned from the `secp256k1`
/// crate is 65 bytes long, that is because it is prefixed by `0x04` to
/// indicate an uncompressed public key; this first byte is ignored when
/// computing the hash.
pub fn to_address(key: Point<Secp256k1>) -> Address {
  let key = key.as_raw();
  public_key_address(key)
}

/// Gets the checksumed address of a public key.
///
/// The public address is defined as the low 20 bytes of the keccak hash of
/// the public key. Note that the public key returned from the `secp256k1`
/// crate is 65 bytes long, that is because it is prefixed by `0x04` to
/// indicate an uncompressed public key; this first byte is ignored when
/// computing the hash.
pub fn to_address_checksum(key: Point<Secp256k1>) -> String {
  checksum(to_address(key))
}

pub fn message_hash<M>(message: M) -> H256
where
  M: AsRef<[u8]>,
{
  let message = message.as_ref();
  let mut eth_message = format!("\x19Ethereum Signed Message:\n{}", message.len()).into_bytes();
  eth_message.extend_from_slice(message);
  keccak256(&eth_message).into()
}

pub fn attach_signature(_signature: &SignatureRecid, _transaction: &TransactionRequest) {
  todo!()
}

pub fn sanitize_signature(signature: &mut SignatureRecid, chain: u32) {
  let s = signature.s.to_bigint();
  let n = Secp256k1Scalar::group_order().clone();
  let half_n = n.div_floor(&BigInt::from(2));
  if s > half_n {
    let ns = n - s;
    signature.s = Scalar::<Secp256k1>::from(&ns);
  }

  if signature.recid <= 3 {
    signature.recid += (chain as u64) * 2 + 35;
  }
}

pub fn to_ethereum_signature<H>(hash: H, signature: &mut SignatureRecid, chain: u32) -> anyhow::Result<Signature>
where
  H: Into<RecoveryMessage>,
{
  // secError
  sanitize_signature(signature, chain);

  let rec = Recovery::new(
    hash,
    signature.recid,
    H256::from_slice(signature.r.to_bytes().as_ref()),
    H256::from_slice(signature.s.to_bytes().as_ref()),
  );

  let (signature, v) = rec.as_signature().context("failed take signature from recoverable")?;

  let mut slice: [u8; 65] = [0u8; 65];

  slice[..64].copy_from_slice(&signature);
  slice[64] = v as u8;

  Ok(Signature::from_slice(&slice))
  // Ok(H520::from_slice(&signature))

  // H512::from(rec.as_signature()
}

pub fn hash_to_bytes(hash: String) -> anyhow::Result<H256> {
  H256::from_str(hash.as_str()).context("hash read")
}

/// Gets the checksummed address of a H160 hash
pub fn checksum(address: Address) -> String {
  let address = format!("{:x}", address);
  let address_hash = format!("{:x}", H256::from(keccak256(address.as_bytes())));

  address
    .char_indices()
    .fold(String::from("0x"), |mut acc, (index, address_char)| {
      let n = u16::from_str_radix(&address_hash[index..index + 1], 16).unwrap();

      if n > 7 {
        // make char uppercase if ith character is 9..f
        acc.push_str(&address_char.to_uppercase().to_string())
      } else {
        // already lowercased
        acc.push(address_char)
      }

      acc
    })
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use web3::types::H160;

  use crate::checksum;

  #[test]
  fn test_checksum() {
    let addr = H160::from_str("0xe0fc04fa2d34a66b779fd5cee748268032a146c0").unwrap();
    let checksummed = checksum(addr);
    assert_eq!(checksummed, "0xe0FC04FA2d34a66B779fd5CEe748268032a146c0");

    let addr = H160::from_str("0xE0FC04FA2D34A66B779FD5CEE748268032A146C0").unwrap();
    let checksummed = checksum(addr);
    assert_eq!(checksummed, "0xe0FC04FA2d34a66B779fd5CEe748268032a146c0");
  }
}
