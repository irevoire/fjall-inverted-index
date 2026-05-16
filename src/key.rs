use std::{convert::Infallible, marker::PhantomData};

use fiole::codec::{Bytes, ComposeCodec, Decode, DecodingVec, Encode, EncodingVec, Fresh, U8};

pub(crate) struct Key<Value> {
    pub value: fjall::Slice,
    pub marker: PhantomData<Value>,
}

impl<Value> Key<Value> {
    pub(crate) fn new(value: fjall::Slice) -> Self {
        Self {
            value,
            marker: PhantomData,
        }
    }
}

pub(crate) struct KeyCodec<Value>(pub PhantomData<Value>);

impl<Value> Encode for KeyCodec<Value> {
    type Item = Key<Value>;
    type Error = Infallible;

    fn encode(
        encoding: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let Self::Item { value, marker: _ } = item;
        Bytes::encode(encoding, &value)
    }
}

impl<Value> Decode for KeyCodec<Value> {
    type Item = Key<Value>;
    type Error = <U8 as Decode>::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        let (level, value) =
            ComposeCodec::<(U8, Bytes)>::decode(bytes).map_err(|err| err.unwrap_C1())?;
        Ok(Key {
            value: value.into(),
            marker: PhantomData,
        })
    }
}
