use std::fmt;

use fiole::codec::{Decode, DecodingVec, Encode};

#[derive(Debug, Clone)]
pub enum Query<'a, Codec: Encode> {
    Empty,
    All,
    Not(Box<Query<'a, Codec>>),
    LessThan(&'a Codec::Item),
    MoreThan(&'a Codec::Item),
    Equal(&'a Codec::Item),
}

impl<'a, Codec: Encode + fmt::Display> Query<'a, Codec> {
    pub fn display_with(&self) -> Result<String, Codec::Error>
    where
        Codec::Item: fmt::Display,
    {
        match self {
            Query::Empty => Ok(format!("[EMPTY]")),
            Query::All => Ok(format!("[ALL]")),
            Query::Not(query) => Ok(format!("[NOT] {}", query.display_with()?)),
            Query::LessThan(value) => Ok(format!("[ITEM] < {value}")),
            Query::MoreThan(value) => Ok(format!("[ITEM] > {value}",)),
            Query::Equal(value) => Ok(format!("[ITEM] = {value}",)),
        }
    }
}
