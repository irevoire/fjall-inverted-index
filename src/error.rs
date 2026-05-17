use std::fmt;

use fiole::codec::{Decode, Encode, U8};

pub enum Error<Codec: Encode + Decode> {
    Fjall(fjall::Error),
    CouldNotEncodeValue(<Codec as Encode>::Error),
    CouldNotDecodeValue(<Codec as Decode>::Error),
    CouldNotDecodeKeyTag(<U8 as Decode>::Error),
    CouldNotEncodeOrDecodeRoaring,
}

impl<Codec> fmt::Display for Error<Codec>
where
    Codec: Encode + Decode,
    <Codec as Encode>::Error: fmt::Display,
    <Codec as Decode>::Error: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::CouldNotEncodeValue(error) => error.fmt(f),
            Error::CouldNotDecodeValue(error) => error.fmt(f),
            Error::Fjall(error) => error.fmt(f),
            Error::CouldNotEncodeOrDecodeRoaring => {
                f.write_str("Internal error, could not encode or decode roaring bitmap")
            }
            Error::CouldNotDecodeKeyTag(error) => error.fmt(f),
        }
    }
}

impl<Codec> fmt::Debug for Error<Codec>
where
    Codec: Encode + Decode,
    <Codec as Encode>::Error: fmt::Debug,
    <Codec as Decode>::Error: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Fjall(error) => f.debug_tuple("Fjall").field(error).finish(),
            Error::CouldNotEncodeValue(encode) => {
                f.debug_tuple("CouldNotEncodeValue").field(encode).finish()
            }
            Error::CouldNotDecodeValue(decode) => {
                f.debug_tuple("CouldNotDecodeValue").field(decode).finish()
            }
            Error::CouldNotEncodeOrDecodeRoaring => {
                f.debug_tuple("CouldNotEncodeOrDecodeRoaring").finish()
            }
            Error::CouldNotDecodeKeyTag(error) => {
                f.debug_tuple("CouldNotDecodeKeyTag").field(error).finish()
            }
        }
    }
}

impl<Codec> std::error::Error for Error<Codec>
where
    Codec: Encode + Decode,
    <Codec as Encode>::Error: std::error::Error,
    <Codec as Decode>::Error: std::error::Error,
{
}
