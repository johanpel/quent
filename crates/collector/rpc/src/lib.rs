// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Collector gRPC types and their binary wire encoding.

use std::marker::PhantomData;

use bytes::{Buf, BufMut};
use tonic::{
    Status,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
};

/// Maximum encoded size of an event batch sent by collector clients.
pub const MAX_EVENT_BATCH_ENCODED_LEN: usize = 4 * 1024 * 1024;

/// Maximum number of events in one batch.
pub const MAX_EVENTS_PER_BATCH: usize = 65_536;

/// Encoded size of the event-count field at the start of a batch.
pub const EVENT_BATCH_HEADER_LEN: usize = size_of::<u32>();

/// Encoded size of the length field before each event payload.
pub const EVENT_LENGTH_HEADER_LEN: usize = size_of::<u32>();

/// A batch of serialized collector events.
///
/// The wire representation is a big-endian `u32` event count followed by a
/// big-endian `u32` length and the raw bytes for each event.
#[derive(Debug, Eq, PartialEq)]
pub struct EventBatch {
    pub events: Vec<Vec<u8>>,
}

/// The empty response returned when the collector stream handler completes.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct CollectResponse;

trait WireMessage: Sized {
    fn encode(self, dst: &mut impl BufMut) -> Result<(), Status>;
    fn decode(src: &mut impl Buf) -> Result<Self, Status>;
}

impl WireMessage for EventBatch {
    fn encode(self, dst: &mut impl BufMut) -> Result<(), Status> {
        if self.events.len() > MAX_EVENTS_PER_BATCH {
            return Err(Status::out_of_range("event batch contains too many events"));
        }
        let event_count = u32::try_from(self.events.len())
            .map_err(|_| Status::out_of_range("too many events in one batch"))?;
        if self
            .events
            .iter()
            .any(|event| event.len() > u32::MAX as usize)
        {
            return Err(Status::out_of_range("event payload exceeds 4 GiB"));
        }

        dst.put_u32(event_count);
        for event in self.events {
            dst.put_u32(event.len() as u32);
            dst.put_slice(&event);
        }
        Ok(())
    }

    fn decode(src: &mut impl Buf) -> Result<Self, Status> {
        if src.remaining() < EVENT_BATCH_HEADER_LEN {
            return Err(Status::invalid_argument(
                "event batch is missing its event count",
            ));
        }

        let event_count = src.get_u32() as usize;
        if event_count > MAX_EVENTS_PER_BATCH {
            return Err(Status::invalid_argument(
                "event batch contains too many events",
            ));
        }
        if event_count > src.remaining() / EVENT_LENGTH_HEADER_LEN {
            return Err(Status::invalid_argument(
                "event batch contains an invalid event count",
            ));
        }

        let mut events = Vec::with_capacity(event_count);
        for _ in 0..event_count {
            if src.remaining() < EVENT_LENGTH_HEADER_LEN {
                return Err(Status::invalid_argument(
                    "event batch is missing an event length",
                ));
            }
            let event_len = src.get_u32() as usize;
            if event_len > src.remaining() {
                return Err(Status::invalid_argument("event payload is truncated"));
            }
            events.push(src.copy_to_bytes(event_len).to_vec());
        }
        if src.has_remaining() {
            return Err(Status::invalid_argument("event batch has trailing bytes"));
        }

        Ok(Self { events })
    }
}

impl WireMessage for CollectResponse {
    fn encode(self, _dst: &mut impl BufMut) -> Result<(), Status> {
        Ok(())
    }

    fn decode(src: &mut impl Buf) -> Result<Self, Status> {
        if src.has_remaining() {
            return Err(Status::invalid_argument(
                "collector response has trailing bytes",
            ));
        }
        Ok(Self)
    }
}

/// Selects the collector wire encoder and decoder for a Tonic client or server.
#[derive(Debug)]
pub struct CollectorCodec<Encode, Decode>(PhantomData<fn() -> (Encode, Decode)>);

impl<Encode, Decode> Default for CollectorCodec<Encode, Decode> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<Encode, Decode> Codec for CollectorCodec<Encode, Decode>
where
    Encode: WireMessage + Send + 'static,
    Decode: WireMessage + Send + 'static,
{
    type Encode = Encode;
    type Decode = Decode;
    type Encoder = CollectorEncoder<Encode>;
    type Decoder = CollectorDecoder<Decode>;

    fn encoder(&mut self) -> Self::Encoder {
        CollectorEncoder(PhantomData)
    }

    fn decoder(&mut self) -> Self::Decoder {
        CollectorDecoder(PhantomData)
    }
}

/// Encodes collector messages into their binary wire representation.
#[derive(Debug)]
pub struct CollectorEncoder<T>(PhantomData<fn(T)>);

impl<T> Encoder for CollectorEncoder<T>
where
    T: WireMessage,
{
    type Item = T;
    type Error = Status;

    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        item.encode(dst)
    }
}

/// Decodes collector messages from their binary wire representation.
#[derive(Debug)]
pub struct CollectorDecoder<T>(PhantomData<fn() -> T>);

impl<T> Decoder for CollectorDecoder<T>
where
    T: WireMessage,
{
    type Item = T;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        T::decode(src).map(Some)
    }
}

include!(concat!(
    env!("OUT_DIR"),
    "/quent.collector.v1alpha.Collector.rs"
));

#[cfg(test)]
mod tests {
    use bytes::BytesMut;
    use tonic::Code;

    use super::{CollectResponse, EventBatch, MAX_EVENTS_PER_BATCH, WireMessage};

    #[test]
    fn event_batch_round_trips() {
        let batch = EventBatch {
            events: vec![vec![], vec![0, 1, 2], vec![255; 1024]],
        };
        let mut encoded = BytesMut::new();

        batch.encode(&mut encoded).unwrap();

        let decoded = EventBatch::decode(&mut encoded).unwrap();
        assert_eq!(
            decoded,
            EventBatch {
                events: vec![vec![], vec![0, 1, 2], vec![255; 1024]],
            }
        );
        assert!(encoded.is_empty());
    }

    #[test]
    fn truncated_event_is_rejected() {
        let mut encoded = BytesMut::from(&[0, 0, 0, 1, 0, 0, 0, 2, 42][..]);

        let error = EventBatch::decode(&mut encoded).unwrap_err();

        assert_eq!(error.code(), Code::InvalidArgument);
    }

    #[test]
    fn missing_event_length_is_rejected() {
        let mut encoded = BytesMut::from(&[0, 0, 0, 2, 0, 0, 0, 4, 1, 2, 3, 4][..]);

        let error = EventBatch::decode(&mut encoded).unwrap_err();

        assert_eq!(error.code(), Code::InvalidArgument);
    }

    #[test]
    fn response_rejects_payload_bytes() {
        let mut encoded = BytesMut::from(&[42][..]);

        let error = CollectResponse::decode(&mut encoded).unwrap_err();

        assert_eq!(error.code(), Code::InvalidArgument);
    }

    #[test]
    fn excessive_event_count_is_rejected_before_decoding_entries() {
        let event_count = (MAX_EVENTS_PER_BATCH as u32 + 1).to_be_bytes();
        let mut encoded = BytesMut::from(&event_count[..]);

        let error = EventBatch::decode(&mut encoded).unwrap_err();

        assert_eq!(error.code(), Code::InvalidArgument);
    }

    #[test]
    fn excessive_event_count_is_rejected_before_encoding() {
        let batch = EventBatch {
            events: vec![Vec::new(); MAX_EVENTS_PER_BATCH + 1],
        };
        let mut encoded = BytesMut::new();

        let error = batch.encode(&mut encoded).unwrap_err();

        assert_eq!(error.code(), Code::OutOfRange);
        assert!(encoded.is_empty());
    }
}
