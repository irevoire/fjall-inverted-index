use std::{convert::Infallible, marker::PhantomData};

use fiole::codec::{Bytes, ComposeCodec, Decode, DecodingVec, Encode, EncodingVec, Fresh, U8};

use crate::error::Error;

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub(crate) enum KeyKind {
    All = 0,
    Entry = 1,
    FinalKind = 255,
}

impl Encode for KeyKind {
    type Item = Self;
    type Error = std::convert::Infallible;

    #[inline]
    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        U8::encode(into, &(*item as u8))
    }
}

pub(crate) enum Key<Value> {
    All,
    Entry(fjall::Slice),
    #[allow(dead_code)]
    Marker(Infallible, PhantomData<Value>),
}

impl<Value> Key<Value> {
    pub fn as_kind(&self) -> KeyKind {
        match self {
            Key::All => KeyKind::All,
            Key::Entry(_) => KeyKind::Entry,
            Key::Marker(..) => unreachable!(),
        }
    }

    pub fn as_entry(&self) -> &fjall::Slice {
        match self {
            Key::Entry(slice) => &slice,
            Key::All => panic!(),
            Key::Marker(..) => panic!(),
        }
    }
}

impl<Value: Encode + Decode> Key<Value> {
    pub(crate) fn new(value: &<Value as Encode>::Item) -> Result<Self, Error<Value>> {
        Ok(Self::Entry(
            Value::encode_alloc(value)
                .map_err(Error::CouldNotEncodeValue)?
                .into_fjall_slice(),
        ))
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
        match item {
            Key::All => U8::encode(encoding, &(item.as_kind() as u8)),
            Key::Entry(slice) => {
                let encoding = U8::encode(encoding, &(item.as_kind() as u8))?;
                Bytes::encode(encoding, &slice)
            }
            Key::Marker(..) => unreachable!(),
        }
    }
}

impl<Value> Decode for KeyCodec<Value> {
    type Item = Key<Value>;
    type Error = <U8 as Decode>::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        let (kind, value) =
            ComposeCodec::<(U8, Bytes)>::decode(bytes).map_err(|err| err.unwrap_C1())?;
        match kind {
            n if n == KeyKind::All as u8 => Ok(Key::All),
            n if n == KeyKind::Entry as u8 => Ok(Key::Entry(value.into())),
            _ => panic!("Unexpected key codec kind"),
        }
    }
}
