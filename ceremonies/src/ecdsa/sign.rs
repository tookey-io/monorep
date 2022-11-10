use anyhow::{anyhow, Context, Result};
use curv::{elliptic::curves::Secp256k1, BigInt};
use derive_more::TryInto;
use futures::{channel::mpsc::channel, FutureExt, Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use round_based::{AsyncProtocol, Msg};
use tss::ecdsa::{
    party_i::SignatureRecid,
    state_machine::{
        keygen::LocalKey,
        sign::{Error as SignError, OfflineProtocolMessage, OfflineStage, PartialSignature, SignManual},
    },
};

#[derive(TryInto)]
#[allow(clippy::large_enum_variant)]
pub enum Messages {
    OfflineStage(Msg<OfflineProtocolMessage>),
    PartialSignature(Msg<PartialSignature>),
}

pub async fn sign<O, I>(
    outgoing: O,
    incoming: I,
    local_share: LocalKey<Secp256k1>,
    parties: Vec<u16>,
    hash: BigInt,
) -> Result<SignatureRecid>
where
    O: Sink<Messages, Error = anyhow::Error>,
    I: Stream<Item = Result<Messages, anyhow::Error>>,
{
    let index = parties
        .iter()
        .position(|p| *p == local_share.i)
        .map(|v| v + 1)
        .context("Not in party")? as u16;

    let number_of_parties = parties.len();

    tokio::pin!(outgoing);

    // let receiver_offline = futures::stream::iter(vec![]);
    let (mut incoming_offline_sender, incoming_offline_receiver) = channel::<Result<_, SignError>>(102400);

    let (outgoing_offline_sender, outgoing_offline_receiver) = channel(102400);

    let (mut incoming_partial_sender, incoming_partial_receiver) = channel::<Result<_, SignError>>(102400);
    let (mut outgoing_partial_sender, outgoing_partial_receiver) = channel(102400);

    let outgoing_offline_receiver = outgoing_offline_receiver.map(Messages::OfflineStage).boxed();
    let outgoing_partial_receiver = outgoing_partial_receiver.map(Messages::PartialSignature).boxed();

    let outgoin_stream = futures::stream::select_all(vec![outgoing_offline_receiver, outgoing_partial_receiver]);

    let send_outgoings = async move {
        tokio::pin!(outgoin_stream);
        while let Some(message) = outgoin_stream.next().await {
            outgoing.send(message).await.context("outgoin sink send")?
        }

        Ok::<_, anyhow::Error>(())
    };

    let split_stream = async move {
        tokio::pin!(incoming);
        while let Some(incoming) = incoming.next().await {
            match incoming {
                Ok(Messages::OfflineStage(msg)) => {
                    incoming_offline_sender
                        .send(Ok(msg))
                        .await
                        .map_err(|e| anyhow!("sending offline incoming: {}", e))?;
                }
                Ok(Messages::PartialSignature(msg)) => {
                    incoming_partial_sender
                        .send(Ok(msg))
                        .await
                        .map_err(|e| anyhow!("sending offline incoming: {}", e))?;
                }
                _ => todo!(),
            }
        }

        Ok::<_, anyhow::Error>(())
    };

    let signature = async move {
        let signing = OfflineStage::new(index, parties, local_share)?;

        let incoming_offline_receiver = incoming_offline_receiver.fuse();
        tokio::pin!(incoming_offline_receiver);
        tokio::pin!(outgoing_offline_sender);

        let mut protocol = AsyncProtocol::new(signing, incoming_offline_receiver, outgoing_offline_sender);

        let completed_offline_stage = protocol
            .run()
            .await
            .map_err(|e| anyhow!("protocol execution terminated with error: {:?}", e))?;

        let (signing, partial_signature) = SignManual::new(hash, completed_offline_stage)?;

        outgoing_partial_sender
            .send(Msg {
                sender: index,
                receiver: None,
                body: partial_signature,
            })
            .await
            .map_err(|e| anyhow!("sending partial incoming: {}", e))?;

        let partial_signatures: Vec<_> = incoming_partial_receiver
            .take(number_of_parties - 1)
            .map_ok(|msg| msg.body)
            .try_collect()
            .await?;

        let signature = signing.complete(&partial_signatures).context("online stage failed")?;

        Ok::<_, anyhow::Error>(signature)
    };

    // let (_, _, result) = futures::select!(send_outgoings, split_stream, signature);
    let result = futures::select! {
        send_outgoings = send_outgoings.fuse() => Err::<SignatureRecid, _>(anyhow!("send outgoins close too early: {:?}", send_outgoings)),
        split_stream = split_stream.fuse() => Err::<SignatureRecid, _>(anyhow!("spliting incomings close too early: {:?}", split_stream)),
        signature = signature.fuse() => signature,
    };

    result
}
