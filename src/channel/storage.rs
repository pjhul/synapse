use rocksdb::{DB, Options};

#[derive(Debug)]
pub struct ChannelStorage {
    // TODO: We only ever have a single instance of our ChannelMap so we don't need a mutex here,
    // but eventually this will need to be thread-safe
    db: DB,
}

impl ChannelStorage {
    pub fn new(path: &str) -> Self {
        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, path).expect("Failed to open RocksDB");
        ChannelStorage { db }
    }

    pub fn create_channel(&self, name: &str) -> Result<(), rocksdb::Error> {
        let channel_exists = self.db.get(name.as_bytes())?;
        if channel_exists.is_none() {
            self.db.put(name.as_bytes(), b"")?;
        }
        Ok(())
    }

    pub fn remove_channel(&self, name: &str) -> Result<(), rocksdb::Error> {
        self.db.delete(name.as_bytes())?;
        Ok(())
    }

    pub fn get_channels(&self) -> Result<Vec<String>, rocksdb::Error> {
        let cf = self.db.cf_handle("default").unwrap();
        let mut channels = Vec::new();
        let iter = self.db.full_iterator_cf(cf, rocksdb::IteratorMode::Start);

        for key_result in iter {
            let (key, _) = key_result?;
            let channel_name = String::from_utf8_lossy(&key).to_string();
            channels.push(channel_name);
        }

        Ok(channels)
    }
}

