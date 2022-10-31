use anyhow::{anyhow, Context, Result};
use curv::{arithmetic::Converter, elliptic::curves::Secp256k1, BigInt};
use derive_more::TryInto;
use futures::{channel::mpsc::channel, Sink, SinkExt, Stream, StreamExt, TryStreamExt, FutureExt};
use round_based::{AsyncProtocol, Msg};
use tss::ecdsa::{
    party_i::SignatureRecid,
    state_machine::{
        keygen::{LocalKey, ProtocolMessage, Error as KeygenError},
    },
};

#[derive(TryInto)]
pub enum Messages {
    Keygen(Msg<ProtocolMessage>),
}

pub async fn keygen<O, I>(
    outgoing: O,
    incoming: I,
    index: u16,
    threshold: u16,
    parties: u16,
) -> Result<LocalKey<Secp256k1>>
where
    O: Sink<Messages, Error = anyhow::Error>,
    I: Stream<Item = Result<Messages, anyhow::Error>>,
{
    tokio::pin!(outgoing);

    // let receiver_offline = futures::stream::iter(vec![]);
    let (mut incoming_offline_sender, incoming_offline_receiver) =
        channel::<Result<_, SignError>>(102400);

    let (outgoing_offline_sender, outgoing_offline_receiver) = channel(102400);

    let outgoing_offline_receiver = outgoing_offline_receiver
        .map(|msg| Messages::OfflineStage(msg))
        .boxed();
        
    let send_outgoings = async move {
        tokio::pin!(outgoing_offline_receiver);
        while let Some(message) = outgoing_offline_receiver.next().await {
            outgoing.send(message).await.context("outgoin sink send")?
        }

        Ok::<_, anyhow::Error>(())
    };

    let split_stream = async move {
        tokio::pin!(incoming);
        while let Some(Messages::Keygen(msg)) = incoming.next().await {
                    incoming_offline_sender
                        .send(Ok(msg))
                        .await
                        .map_err(|e| anyhow!("sending offline incoming: {}", e))?;
        }

        Ok::<_, anyhow::Error>(())
    };

    let local_key = async move {
        let keygen = Keygen::new(index, threshold, parties)?;

        let incoming_offline_receiver = incoming_offline_receiver.fuse();
        tokio::pin!(incoming_offline_receiver);
        tokio::pin!(outgoing_offline_sender);

        let mut protocol =
            AsyncProtocol::new(signing, incoming_offline_receiver, outgoing_offline_sender);

        let completed_offline_stage = protocol
            .run()
            .await
            .map_err(|e| anyhow!("protocol execution terminated with error: {}", e))?;

        let (signing, partial_signature) =
            SignManual::new(hash, completed_offline_stage)?;

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

        let signature = signing
            .complete(&partial_signatures)
            .context("online stage failed")?;

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
