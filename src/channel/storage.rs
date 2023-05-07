use rocksdb::{DB, Options};
use std::sync::Mutex;

#[derive(Debug)]
pub struct ChannelStorage {
    db: Mutex<DB>,
}

impl ChannelStorage {
    pub fn new(path: &str) -> Self {
        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, path).expect("Failed to open RocksDB");
        ChannelStorage { db: Mutex::new(db) }
    }

    pub fn create_channel(&self, name: &str) -> Result<(), rocksdb::Error> {
        let mut db = self.db.lock().unwrap();
        let channel_exists = db.get(name.as_bytes())?;
        if channel_exists.is_none() {
            db.put(name.as_bytes(), b"")?;
        }
        Ok(())
    }

    pub fn get_channels(&self) -> Result<Vec<String>, rocksdb::Error> {
        let db = self.db.lock().unwrap();
        let cf = db.cf_handle("default").unwrap();
        let mut channels = Vec::new();
        let iter = db.full_iterator_cf(cf, rocksdb::IteratorMode::Start);

        for key_result in iter {
            let (key, _) = key_result?;
            let channel_name = String::from_utf8_lossy(&key).to_string();
            channels.push(channel_name);
        }

        Ok(channels)
    }
}

