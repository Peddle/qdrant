use crate::entry::entry_point::{OperationError, OperationResult};
use atomic_refcell::AtomicRefCell;
use rocksdb::{LogLevel, Options, WriteOptions, DB};
use std::path::Path;
use std::sync::Arc;

const DB_CACHE_SIZE: usize = 10 * 1024 * 1024; // 10 mb
const DB_MAX_LOG_SIZE: usize = 1024 * 1024; // 1 mb
const DB_MAX_OPEN_FILES: usize = 256;

pub const DB_VECTOR_CF: &str = "vector";
pub const DB_PAYLOAD_CF: &str = "payload";
pub const DB_MAPPING_CF: &str = "mapping";
pub const DB_VERSIONS_CF: &str = "version";

pub const FIXED_KEY: &[u8] = &[1; 32];
pub const FIXED_VALUE: &[u8] = &[0];

pub const LIST_CF_ERROR: &str = "version";

pub struct Database {
    db: DB,
    is_appendable: bool,
}

pub struct DatabaseColumn {
    pub database: Arc<AtomicRefCell<Database>>,
    pub column_name: String,
    pub put_fixed_key: AtomicRefCell<bool>,
}

pub enum DatabaseIterationResult<T> {
    Break(OperationResult<T>),
    Continue,
}

impl Database {
    pub fn new(path: &Path, default_columns: bool, is_appendable: bool) -> OperationResult<Self> {
        let column_families: Vec<String> = if default_columns {
            vec![
                DB_VECTOR_CF.to_string(),
                DB_PAYLOAD_CF.to_string(),
                DB_MAPPING_CF.to_string(),
                DB_VERSIONS_CF.to_string(),
            ]
        } else {
            let db_file = path.join("CURRENT");
            if db_file.exists() {
                DB::list_cf(&Self::get_options(), path)
                    .map_err(|err| OperationError::service_error(&format!("RocksDB list_cf error: {}", err)))?
            } else {
                vec![]
            }
        };
        let db = DB::open_cf(&Self::get_options(), path, &column_families)
            .map_err(|err| OperationError::service_error(&format!("RocksDB open_cf error: {}", err)))?;
        Ok(Self { db, is_appendable })
    }

    fn get_options() -> Options {
        let mut options: Options = Options::default();
        options.set_write_buffer_size(DB_CACHE_SIZE);
        options.create_if_missing(true);
        options.set_log_level(LogLevel::Error);
        options.set_recycle_log_file_num(2);
        options.set_max_log_file_size(DB_MAX_LOG_SIZE);
        options.create_missing_column_families(true);
        options.set_max_open_files(DB_MAX_OPEN_FILES as i32);
        #[cfg(debug_assertions)]
        {
            options.set_paranoid_checks(true);
        }
        options
    }
}

impl DatabaseColumn {
    pub fn new(database: Arc<AtomicRefCell<Database>>, column_name: &str) -> Self {
        Self {
            database,
            column_name: column_name.to_string(),
            put_fixed_key: AtomicRefCell::new(false),
        }
    }

    pub fn put<K, V>(&self, key: K, value: V) -> OperationResult<()>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        if key.as_ref() == FIXED_KEY {
            return Err(OperationError::service_error(
                "Rocksdb: cannot use FIXED_KEY value",
            ));
        }
        let db = self.database.borrow();
        let cf_handle = db
            .db
            .cf_handle(&self.column_name)
            .ok_or_else(|| OperationError::service_error("COLUMN"))?;
        db.db
            .put_cf_opt(cf_handle, key, value, &Self::get_write_options())
            .map_err(|err| OperationError::service_error(&format!("RocksDB put_cf error: {}", err)))?;
        if *self.put_fixed_key.borrow() {
            db.db
                .put_cf_opt(
                    cf_handle,
                    FIXED_KEY,
                    FIXED_VALUE,
                    &Self::get_write_options(),
                )
                .map_err(|err| OperationError::service_error(&format!("RocksDB put_cf (fixed key) error: {}", err)))?;
            *self.put_fixed_key.borrow_mut() = false;
        }
        Ok(())
    }

    pub fn get_pinned<T, F>(&self, key: &[u8], f: F) -> OperationResult<Option<T>>
    where
        F: FnOnce(&[u8]) -> T,
    {
        let db = self.database.borrow();
        let cf_handle = db
            .db
            .cf_handle(&self.column_name)
            .ok_or_else(|| OperationError::service_error("COLUMN"))?;
        let result = db
            .db
            .get_pinned_cf(cf_handle, key)
            .map_err(|err| OperationError::service_error(&format!("RocksDB get_pinned_cf error: {}", err)))?
            .map(|value| f(&value));
        Ok(result)
    }

    pub fn remove<K>(&self, key: K) -> OperationResult<()>
    where
        K: AsRef<[u8]>,
    {
        let db = self.database.borrow();
        let cf_handle = db
            .db
            .cf_handle(&self.column_name)
            .ok_or_else(|| OperationError::service_error("COLUMN"))?;
        db.db
            .delete_cf(cf_handle, key)
            .map_err(|err| OperationError::service_error(&format!("RocksDB delete_cf error: {}", err)))?;
        Ok(())
    }

    pub fn iterate_over_column_family<T, F>(&self, mut f: F) -> OperationResult<T>
    where
        F: FnMut((&[u8], &[u8])) -> DatabaseIterationResult<T>,
        T: Default,
    {
        let db = self.database.borrow();
        let cf_handle = db
            .db
            .cf_handle(&self.column_name)
            .ok_or_else(|| OperationError::service_error("COLUMN"))?;

        let mut iter = db.db.raw_iterator_cf(&cf_handle);
        iter.seek_to_first();

        while iter.valid() {
            let key = iter
                .key()
                .ok_or_else(|| OperationError::service_error("RocksDB iterator invalid key"))?;
            if key != FIXED_KEY {
                let value = iter
                    .value()
                    .ok_or_else(|| OperationError::service_error("RocksDB iterator invalid value"))?;
                match f((key, value)) {
                    DatabaseIterationResult::Break(result) => return result,
                    DatabaseIterationResult::Continue => {}
                }
            }
            iter.next();
        }
        Ok(T::default())
    }

    pub fn flush(&self) -> OperationResult<()> {
        let db = self.database.borrow();
        let column_family = db.db.cf_handle(&self.column_name).ok_or_else(|| {
            OperationError::service_error(&format!(
                "COLUMN"
            ))
        })?;

        db.db
            .flush_cf(column_family)
            .map_err(|err| OperationError::service_error(&format!("RocksDB flush_cf error: {}", err)))?;
        if db.is_appendable {
            *self.put_fixed_key.borrow_mut() = true;
        }
        Ok(())
    }

    pub fn create_column_family_if_not_exists(&self) -> OperationResult<()> {
        let mut db = self.database.borrow_mut();
        if db.db.cf_handle(&self.column_name).is_none() {
            db.db
                .create_cf(&self.column_name, &Database::get_options())
                .map_err(|err| OperationError::service_error(&format!("RocksDB create_cf error: {}", err)))?;
        }
        Ok(())
    }

    pub fn recreate_column_family(&self) -> OperationResult<()> {
        self.remove_column_family()?;
        self.create_column_family_if_not_exists()
    }

    pub fn remove_column_family(&self) -> OperationResult<()> {
        let mut db = self.database.borrow_mut();
        if db.db.cf_handle(&self.column_name).is_some() {
            db.db
                .drop_cf(&self.column_name)
                .map_err(|err| OperationError::service_error(&format!("RocksDB drop_cf error: {}", err)))?;
        }
        Ok(())
    }

    pub fn has_column_family(&self) -> OperationResult<bool> {
        let db = self.database.borrow();
        Ok(db.db.cf_handle(&self.column_name).is_some())
    }

    fn get_write_options() -> WriteOptions {
        let mut write_options = WriteOptions::default();
        write_options.set_sync(false);
        write_options.disable_wal(true);
        write_options
    }
}
