use std::ops::Bound;

use fiole::{
    codec::{Bytes, Decode, Encode, RoaringBitmapCodec, Unspecified},
    Keyspace, Readable, Wtxn,
};
use roaring::RoaringBitmap;

use crate::{
    error::Error,
    key::{Key, KeyCodec, KeyKind},
};

mod error;
mod key;
mod query;

pub use query::Filter;

pub struct SkipList<Value: Encode + Decode> {
    ks: Keyspace<KeyCodec<Value>, RoaringBitmapCodec>,
}

impl<Value: Encode + Decode> SkipList<Value> {
    pub fn new(ks: Keyspace<Unspecified, Unspecified>) -> Self {
        Self {
            ks: ks.remap_types(),
        }
    }

    pub fn insert<'a>(
        &self,
        wtxn: &mut Wtxn,
        docid: u32,
        values: impl IntoIterator<Item = &'a <Value as Encode>::Item>,
    ) -> Result<(), Error<Value>>
    where
        <Value as Encode>::Item: 'a,
    {
        // update all ids
        let mut ids = self
            .ks
            .get(wtxn, &Key::All)
            .map_err(|err| match err {
                fiole::Error::Fjall(error) => Error::Fjall(error),
                fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
            })?
            .unwrap_or_default();
        ids.insert(docid);
        self.ks
            .insert(wtxn, &Key::All, &ids)
            .map_err(|err| match err {
                fiole::Error::Fjall(error) => Error::Fjall(error),
                fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
            })?;

        // Insert the id in all values
        for value in values {
            let key = Key::new(value)?;
            let mut ids = self
                .ks
                .get(wtxn, &key)
                .map_err(|err| match err {
                    fiole::Error::Fjall(error) => Error::Fjall(error),
                    fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
                })?
                .unwrap_or_default();
            ids.insert(docid);
            self.ks.insert(wtxn, &key, &ids).map_err(|err| match err {
                fiole::Error::Fjall(error) => Error::Fjall(error),
                fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
            })?;
        }
        Ok(())
    }

    pub fn filter(
        &self,
        rtxn: &impl Readable,
        query: &Filter<'_, Value>,
    ) -> Result<RoaringBitmap, Error<Value>> {
        match query {
            Filter::None => Ok(RoaringBitmap::new()),
            Filter::All => Ok(self
                .ks
                .get(rtxn, &Key::All)
                .map_err(|err| match err {
                    fiole::Error::Fjall(error) => Error::Fjall(error),
                    fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
                })?
                .unwrap_or_default()),
            Filter::Not(query) => Ok(self.filter(rtxn, &Filter::All)? - self.filter(rtxn, &query)?),
            Filter::Or(queries) => {
                let all_len = self.filter(rtxn, &Filter::All)?.len();
                let mut ret = RoaringBitmap::new();
                for query in queries.iter() {
                    ret |= self.filter(rtxn, query)?;
                    if ret.len() == all_len {
                        break;
                    }
                }
                Ok(ret)
            }
            Filter::And(queries) => {
                let mut iter = queries.iter();
                let mut ret = if let Some(query) = iter.next() {
                    self.filter(rtxn, query)?
                } else {
                    RoaringBitmap::new()
                };
                for query in iter {
                    if ret.is_empty() {
                        break;
                    }
                    ret &= self.filter(rtxn, query)?;
                }
                Ok(ret)
            }
            Filter::LessThan(value) => self.filter(rtxn, &Filter::range(..value)),
            Filter::LessThanOrEqual(value) => self.filter(rtxn, &Filter::range(..=value)),
            Filter::MoreThan(value) => self.filter(
                rtxn,
                &Filter::range((Bound::Excluded(value), Bound::Unbounded)),
            ),
            Filter::MoreThanOrEqual(value) => self.filter(rtxn, &Filter::range(value..)),
            Filter::Equal(value) => {
                let slice = Value::encode_alloc(value)
                    .map_err(Error::CouldNotEncodeValue)?
                    .finish();

                self.ks
                    .remap_key_type::<Bytes>()
                    .get(rtxn, &slice)
                    .map_err(|err| match err {
                        fiole::Error::Fjall(error) => Error::Fjall(error),
                        fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
                    })
                    .map(|bitmap| bitmap.unwrap_or_default())
            }
            Filter::Range((start, end)) => {
                let start = match start {
                    Bound::Included(start) => Bound::Included(
                        KeyCodec::encode_alloc(&Key::<Value>::Entry(
                            Value::encode_alloc(&start)
                                .map_err(Error::CouldNotEncodeValue)?
                                .into_fjall_slice(),
                        ))
                        .map_err(|err| match err {})?
                        .finish(),
                    ),
                    Bound::Excluded(start) => Bound::Excluded(
                        KeyCodec::encode_alloc(&Key::<Value>::Entry(
                            Value::encode_alloc(&start)
                                .map_err(Error::CouldNotEncodeValue)?
                                .into_fjall_slice(),
                        ))
                        .map_err(|err| match err {})?
                        .finish(),
                    ),
                    Bound::Unbounded => Bound::Included(vec![KeyKind::Entry as u8]),
                };
                let end = match end {
                    Bound::Included(end) => Bound::Included(
                        KeyCodec::encode_alloc(&Key::<Value>::Entry(
                            Value::encode_alloc(&end)
                                .map_err(Error::CouldNotEncodeValue)?
                                .into_fjall_slice(),
                        ))
                        .map_err(|err| match err {})?
                        .finish(),
                    ),
                    Bound::Excluded(end) => Bound::Excluded(
                        KeyCodec::encode_alloc(&Key::<Value>::Entry(
                            Value::encode_alloc(&end)
                                .map_err(Error::CouldNotEncodeValue)?
                                .into_fjall_slice(),
                        ))
                        .map_err(|err| match err {})?
                        .finish(),
                    ),
                    Bound::Unbounded => Bound::Excluded(vec![KeyKind::FinalKind as u8]),
                };

                self.ks
                    .remap_key_type::<Bytes>()
                    .range(
                        rtxn,
                        &(
                            start.as_ref().map(|s| s.as_slice()),
                            end.as_ref().map(|s| s.as_slice()),
                        ),
                    )
                    .map_err(|err| match err {})?
                    .remap_key_type::<KeyCodec<Bytes>>()
                    .map(|guard| {
                        println!("Called on somethin");
                        guard.into_inner().map_err(|err| match err {
                            fiole::Error::Fjall(error) => Error::Fjall(error),
                            fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
                            fiole::Error::Key(error) => Error::CouldNotDecodeKeyTag(error),
                        })
                    })
                    .try_fold(RoaringBitmap::new(), |acc, kv| Ok(acc | kv?.1))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::fmt;

    use fiole::{
        byteorder::BE,
        codec::{Decode, Encode, U32},
    };
    use fjall::KeyspaceCreateOptions;
    use tempfile::TempDir;

    use crate::{query::Filter, SkipList};

    struct TestDb<Value: Encode + Decode> {
        db: fiole::Database,
        ks: SkipList<Value>,
        _dir: TempDir,
    }

    impl<Value> TestDb<Value>
    where
        Value: Encode + Decode,
        <Value as Encode>::Item: fmt::Debug,
        <Value as Encode>::Error: std::error::Error,
        <Value as Decode>::Item: fmt::Debug,
        <Value as Decode>::Error: std::error::Error,
    {
        fn create() -> TestDb<Value> {
            let dir = tempfile::tempdir().unwrap();
            let db = fiole::Database::builder(dir.path()).unwrap();
            let ks = db
                .keyspace("skip list", KeyspaceCreateOptions::default)
                .unwrap();
            let sl = SkipList::new(ks);
            TestDb {
                db,
                ks: sl,
                _dir: dir,
            }
        }
    }

    #[test]
    fn insert_10_elements_and_simple_queries() {
        let db = TestDb::<U32<BE>>::create();
        let mut wtxn = db.db.write_tx().unwrap();
        for i in 0..10 {
            db.ks.insert(&mut wtxn, i, [&i]).unwrap();
        }

        let ret = db.ks.filter(&wtxn, &Filter::None).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        let ret = db.ks.filter(&wtxn, &Filter::All).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]>");

        let ret = db.ks.filter(&wtxn, &Filter::Equal(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        let ret = db.ks.filter(&wtxn, &Filter::range(&4..=&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[4, 5]>");

        let ret = db.ks.filter(&wtxn, &Filter::range(&4..&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[4]>");

        let ret = db.ks.filter(&wtxn, &Filter::range(&4..&4)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        let ret = db.ks.filter(&wtxn, &Filter::range(&4..=&4)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[4]>");

        let ret = db.ks.filter(&wtxn, &Filter::range(&4..=&3)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        let ret = db.ks.filter(&wtxn, &Filter::range(..&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4]>");

        let ret = db.ks.filter(&wtxn, &Filter::range(&6..)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[6, 7, 8, 9]>");

        let ret = db.ks.filter(&wtxn, &Filter::LessThan(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4]>");

        let ret = db.ks.filter(&wtxn, &Filter::LessThanOrEqual(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 5]>");

        let ret = db.ks.filter(&wtxn, &Filter::MoreThan(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[6, 7, 8, 9]>");

        let ret = db.ks.filter(&wtxn, &Filter::MoreThanOrEqual(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[5, 6, 7, 8, 9]>");

        let ret = db
            .ks
            .filter(&wtxn, &Filter::Not(Box::new(Filter::MoreThan(&5))))
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 5]>");

        // --- AND

        // And with no overlap should return nothing
        let ret = db
            .ks
            .filter(
                &wtxn,
                &Filter::And(vec![Filter::LessThan(&5), Filter::MoreThan(&5)]),
            )
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        // And with overlap should return only the overlap
        let ret = db
            .ks
            .filter(
                &wtxn,
                &Filter::And(vec![Filter::LessThan(&6), Filter::MoreThan(&4)]),
            )
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[5]>");

        // And with an empty bitmap shouldn't even evaluate the next queries
        let ret = db
            .ks
            .filter(
                &wtxn,
                &Filter::And(vec![
                    Filter::None,
                    Filter::All,
                    Filter::LessThan(&6),
                    Filter::MoreThan(&4),
                ]),
            )
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        // --- OR

        // Or with no overlap should return the addition of both parts
        let ret = db
            .ks
            .filter(
                &wtxn,
                &Filter::Or(vec![Filter::LessThan(&5), Filter::MoreThan(&5)]),
            )
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 6, 7, 8, 9]>");

        // And with overlap should return only the result of both request
        let ret = db
            .ks
            .filter(
                &wtxn,
                &Filter::Or(vec![Filter::LessThan(&6), Filter::MoreThan(&4)]),
            )
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]>");

        // Or with a full bitmap shouldn't even evaluate the next queries
        let ret = db
            .ks
            .filter(
                &wtxn,
                &Filter::Or(vec![
                    Filter::All,
                    Filter::None,
                    Filter::LessThan(&6),
                    Filter::MoreThan(&4),
                ]),
            )
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]>");
    }
}
