use std::{
    fmt,
    ops::{Bound, RangeBounds},
};

use fiole::codec::Encode;

#[derive(Debug, Clone)]
pub enum Query<'a, Codec: Encode> {
    None,
    All,
    Not(Box<Query<'a, Codec>>),
    Or(Vec<Query<'a, Codec>>),
    And(Vec<Query<'a, Codec>>),
    LessThan(&'a Codec::Item),
    MoreThan(&'a Codec::Item),
    Equal(&'a Codec::Item),
    Range((Bound<&'a Codec::Item>, Bound<&'a Codec::Item>)),
}

impl<'a, Codec: Encode> Query<'a, Codec> {
    pub fn range<R: RangeBounds<&'a Codec::Item> + 'a>(range: R) -> Self {
        Query::Range((range.start_bound().cloned(), range.end_bound().cloned()))
    }
}

impl<'a, Codec: Encode + fmt::Display> Query<'a, Codec> {
    pub fn display_with(&self) -> Result<String, Codec::Error>
    where
        Codec::Item: fmt::Display,
    {
        match self {
            Query::None => Ok(format!("[NONE]")),
            Query::All => Ok(format!("[ALL]")),
            Query::Not(query) => Ok(format!("[NOT] {}", query.display_with()?)),
            Query::Or(queries) => {
                let mut output = String::new();
                for (idx, query) in queries.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(" [OR] ");
                    }
                    output.push_str(&query.display_with()?);
                }

                Ok(output)
            }
            Query::And(queries) => {
                let mut output = String::new();
                for (idx, query) in queries.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(" [AND] ");
                    }
                    output.push_str(&query.display_with()?);
                }

                Ok(output)
            }
            Query::LessThan(value) => Ok(format!("[ITEM] < {value}")),
            Query::MoreThan(value) => Ok(format!("[ITEM] > {value}",)),
            Query::Equal(value) => Ok(format!("[ITEM] = {value}",)),
            Query::Range((start, end)) => {
                let mut ret = String::new();
                match start {
                    Bound::Included(start) => ret.push_str(&format!("{start}")),
                    Bound::Excluded(start) => ret.push_str(&format!("{start}-")),
                    Bound::Unbounded => (),
                }
                ret.push_str("..");
                match end {
                    Bound::Included(end) => ret.push_str(&format!("={end}")),
                    Bound::Excluded(end) => ret.push_str(&format!("{end}")),
                    Bound::Unbounded => (),
                }
                Ok(ret)
            }
        }
    }
}
