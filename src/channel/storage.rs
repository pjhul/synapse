use rocksdb::{ColumnFamilyDescriptor, Options, DB};

use super::Channel;

#[derive(Debug)]
pub struct ChannelStorage {
    // TODO: We only ever have a single instance of our ChannelMap so we don't need a mutex here,
    // but eventually this will need to be thread-safe
    db: DB,
}

pub trait Storage {
    fn new(path: &str) -> Self;
    fn create_channel(&self, channel: &Channel) -> Result<(), rocksdb::Error>;
    fn remove_channel(&self, name: &str) -> Result<(), rocksdb::Error>;
    fn get_channels(&self) -> Result<Vec<Channel>, rocksdb::Error>;
}

// Maybe it even makes sense to have some code here that switches between different storage
// implementations based on the test state so mocking is completely transparent
// Example:
// #[cfg(test)]
// ... mock implementation
// #[cfg(not(test))]
// ... real implementation

impl Storage for ChannelStorage {
    fn new(path: &str) -> Self {
        let mut options = Options::default();
        options.create_if_missing(true);

        let cf_descriptor = ColumnFamilyDescriptor::new("default", options.clone());
        let db = DB::open_cf_descriptors(&options, path, vec![cf_descriptor])
            .expect("Failed to open RocksDB");

        ChannelStorage { db }
    }

    fn create_channel(&self, channel: &Channel) -> Result<(), rocksdb::Error> {
        let channel_exists = self.db.get(channel.name.as_bytes())?;

        if channel_exists.is_none() {
            self.db.put(channel.name.as_bytes(), channel)?;
        }

        Ok(())
    }

    fn remove_channel(&self, name: &str) -> Result<(), rocksdb::Error> {
        self.db.delete(name.as_bytes())?;
        Ok(())
    }

    fn get_channels(&self) -> Result<Vec<Channel>, rocksdb::Error> {
        let cf = self.db.cf_handle("default").unwrap();
        let mut channels = Vec::new();
        let iter = self.db.full_iterator_cf(cf, rocksdb::IteratorMode::Start);

        for key_result in iter {
            let (_, channel_bytes) = key_result?;

            channels.push(Channel::from(channel_bytes));
        }

        Ok(channels)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct MockChannelStorage {}

    impl Storage for MockChannelStorage {
        fn new(_path: &str) -> Self {
            MockChannelStorage {}
        }

        fn create_channel(&self, _name: &Channel) -> Result<(), rocksdb::Error> {
            Ok(())
        }

        fn remove_channel(&self, _name: &str) -> Result<(), rocksdb::Error> {
            Ok(())
        }

        fn get_channels(&self) -> Result<Vec<Channel>, rocksdb::Error> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_create_channel() {
        // TODO: This should get cleaned up after the test
        let storage = ChannelStorage::new("/tmp/test_create_channel");
        let channel = Channel::new(String::from("test"), None, false);

        storage.create_channel(&channel).unwrap();
        let channels = storage.get_channels().unwrap();

        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "test");
        assert!(channels[0].auth.is_none());
    }

    #[test]
    fn test_remove_channel() {
        let storage = ChannelStorage::new("/tmp/test_remove_channel");
        let channel = Channel::new(String::from("test"), None, false);

        storage.create_channel(&channel).unwrap();
        storage.remove_channel("test").unwrap();

        let channels = storage.get_channels().unwrap();
        assert_eq!(channels.len(), 0);
    }
}
