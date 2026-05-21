use std::{
    fmt,
    ops::{Bound, RangeBounds},
};

use fiole::codec::Encode;

#[derive(Debug, Clone)]
pub enum Filter<'a, Codec: Encode> {
    None,
    All,
    Not(Box<Filter<'a, Codec>>),
    Or(Vec<Filter<'a, Codec>>),
    And(Vec<Filter<'a, Codec>>),
    LessThan(&'a Codec::Item),
    LessThanOrEqual(&'a Codec::Item),
    MoreThan(&'a Codec::Item),
    MoreThanOrEqual(&'a Codec::Item),
    Equal(&'a Codec::Item),
    Range((Bound<&'a Codec::Item>, Bound<&'a Codec::Item>)),
}

impl<'a, Codec: Encode> Filter<'a, Codec> {
    pub fn range<R: RangeBounds<&'a Codec::Item> + 'a>(range: R) -> Self {
        Filter::Range((range.start_bound().cloned(), range.end_bound().cloned()))
    }
}

impl<'a, Codec: Encode + fmt::Display> Filter<'a, Codec> {
    pub fn display_with(&self) -> Result<String, Codec::Error>
    where
        Codec::Item: fmt::Display,
    {
        match self {
            Filter::None => Ok(format!("[NONE]")),
            Filter::All => Ok(format!("[ALL]")),
            Filter::Not(query) => Ok(format!("[NOT] {}", query.display_with()?)),
            Filter::Or(queries) => {
                let mut output = String::new();
                for (idx, query) in queries.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(" [OR] ");
                    }
                    output.push_str(&query.display_with()?);
                }

                Ok(output)
            }
            Filter::And(queries) => {
                let mut output = String::new();
                for (idx, query) in queries.iter().enumerate() {
                    if idx != 0 {
                        output.push_str(" [AND] ");
                    }
                    output.push_str(&query.display_with()?);
                }

                Ok(output)
            }
            Filter::LessThan(value) => Ok(format!("[ITEM] < {value}")),
            Filter::LessThanOrEqual(value) => Ok(format!("[ITEM] <= {value}")),
            Filter::MoreThan(value) => Ok(format!("[ITEM] > {value}",)),
            Filter::MoreThanOrEqual(value) => Ok(format!("[ITEM] >= {value}",)),
            Filter::Equal(value) => Ok(format!("[ITEM] = {value}",)),
            Filter::Range((start, end)) => {
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
