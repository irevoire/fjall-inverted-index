use fiole::{
    codec::{Bytes, Decode, Encode, RoaringBitmapCodec, Unspecified},
    Keyspace, Readable, Wtxn,
};
use roaring::RoaringBitmap;

use crate::{
    error::Error,
    key::{Key, KeyCodec, KeyKind},
    query::Query,
};

mod error;
mod key;
mod query;

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

    pub fn query(
        &self,
        rtxn: &impl Readable,
        query: &Query<'_, Value>,
    ) -> Result<RoaringBitmap, Error<Value>> {
        match query {
            Query::None => Ok(RoaringBitmap::new()),
            Query::All => Ok(self
                .ks
                .get(rtxn, &Key::All)
                .map_err(|err| match err {
                    fiole::Error::Fjall(error) => Error::Fjall(error),
                    fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
                })?
                .unwrap_or_default()),
            Query::Not(query) => Ok(self.query(rtxn, &Query::All)? - self.query(rtxn, &query)?),
            Query::LessThan(value) => {
                let slice = Value::encode_alloc(value)
                    .map_err(Error::CouldNotEncodeValue)?
                    .finish();
                self.ks
                    .remap_key_type::<KeyKind>()
                    .prefix(rtxn, &KeyKind::Entry)
                    .map_err(|err| match err {})?
                    .remap_key_type::<KeyCodec<Value>>()
                    .map(|guard| {
                        guard.into_inner().map_err(|err| match err {
                            fiole::Error::Fjall(error) => Error::Fjall(error),
                            fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
                            fiole::Error::Key(value) => Error::CouldNotDecodeKeyTag(value),
                        })
                    })
                    // in case of error we still want to take the value to return it in the fold
                    .take_while(|kv| kv.as_ref().map_or(true, |(key, _)| key.as_entry() < &slice))
                    .try_fold(RoaringBitmap::new(), |acc, kv| Ok(acc | kv?.1))
            }
            Query::MoreThan(value) => {
                let slice = Value::encode_alloc(value)
                    .map_err(Error::CouldNotEncodeValue)?
                    .finish();
                self.ks
                    .remap_key_type::<KeyKind>()
                    .prefix(rtxn, &KeyKind::Entry)
                    .map_err(|err| match err {})?
                    .remap_key_type::<KeyCodec<Value>>()
                    .rev()
                    .map(|guard| {
                        guard.into_inner().map_err(|err| match err {
                            fiole::Error::Fjall(error) => Error::Fjall(error),
                            fiole::Error::Value(_) => Error::CouldNotEncodeOrDecodeRoaring,
                            fiole::Error::Key(error) => Error::CouldNotDecodeKeyTag(error),
                        })
                    })
                    // in case of error we still want to take the value to return it in the fold
                    .take_while(|kv| kv.as_ref().map_or(true, |(key, _)| key.as_entry() > &slice))
                    .try_fold(RoaringBitmap::new(), |acc, kv| Ok(acc | kv?.1))
            }
            Query::Equal(value) => {
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

    use crate::{query::Query, SkipList};

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

        let ret = db.ks.query(&wtxn, &Query::None).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        let ret = db.ks.query(&wtxn, &Query::All).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]>");

        let ret = db.ks.query(&wtxn, &Query::Equal(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[]>");

        let ret = db.ks.query(&wtxn, &Query::LessThan(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4]>");

        let ret = db.ks.query(&wtxn, &Query::MoreThan(&5)).unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[6, 7, 8, 9]>");

        let ret = db
            .ks
            .query(&wtxn, &Query::Not(Box::new(Query::MoreThan(&5))))
            .unwrap();
        insta::assert_debug_snapshot!(ret, @"RoaringBitmap<[0, 1, 2, 3, 4, 5]>");
    }
}
