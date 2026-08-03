/*!
This module contains the [`Data`] struct which is used to access persistent data stored in the file
system.
*/
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind as IOErrorKind;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use common::paths::{ensure_insh_dir_exists, INSH_DIR, INSH_FILES_PERMS};

use fslock::LockFile;

use serde::{Deserialize, Serialize};

lazy_static! {

    /// The file path for user data.
    static ref DATA_FILE_PATH: PathBuf = {
        let mut path: PathBuf = INSH_DIR.clone();
        path.push("data.yaml");
        path
    };

    /// The file path for the lock file on data.
    static ref DATA_LOCK_FILE_PATH: PathBuf = {
        let mut path: PathBuf = INSH_DIR.clone();
        path.push("data.lock");
        path
    };
}

/// Peristent data.
#[derive(Serialize, Deserialize)]
pub struct Data {
    /// Used to synchronize access to the data.
    #[serde(skip, default = "get_lock_file")]
    lock: LockFile,

    /// Data related to searching for text in files.
    pub searcher: SearcherData,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            lock: get_lock_file(),
            searcher: SearcherData::default(),
        }
    }
}

/// Get the lock file object.
fn get_lock_file() -> LockFile {
    ensure_insh_dir_exists();

    // NOTE: The lock file is created w/ the permissions -rw-r--r--. It would be nice if we could
    // change tell it to create it w/ -rw------- but it doesn't look like it has that capability.
    // We could change the perms after it is created but this is probably fine for now.
    let mut lock_file = LockFile::open(&*DATA_LOCK_FILE_PATH).unwrap();
    lock_file.lock_with_pid().unwrap();
    lock_file
}

impl Data {
    /// Acquire the lock to the data file.
    #[allow(dead_code)]
    pub fn acquire(&mut self) {
        self.lock = get_lock_file();
    }

    /// Return if the lock to the data file is owned.
    fn has_lock(&self) -> bool {
        self.lock.owns_lock()
    }

    /// Release the lock around the data file.
    pub fn release(&mut self) {
        self.lock.unlock().unwrap();
    }

    /// Read the data from the file system. If the data file does not exist, then return the default
    /// data.
    ///
    /// This also aquires a lock on the data.
    pub fn read() -> Self {
        let file: File = match File::open(&*DATA_FILE_PATH) {
            Ok(file) => file,
            Err(error) => match error.kind() {
                IOErrorKind::NotFound => {
                    return Data::default();
                }
                error => {
                    panic!(
                        "Could not read stored data. Encounted the following error: {}",
                        error
                    );
                }
            },
        };

        let data: Data = match serde_yaml::from_reader(file) {
            Ok(data) => data,
            Err(error) => {
                panic!(
                    "Could not parse stored data. Encounted the following error: {}",
                    error
                );
            }
        };

        data
    }

    /// Serialize and write the data to a file.
    ///
    /// For now, the data is written as YAML, but speed and size comparisons should be made for
    /// different data formats at some point.
    pub fn write(&mut self) {
        if !self.has_lock() {
            panic!("Cannot write the data because it is not locked.");
        }

        let file: File = OpenOptions::new()
            .write(true)
            .create(true)
            .mode(INSH_FILES_PERMS)
            .open(&*DATA_FILE_PATH)
            .expect("Cannot write persistent data because the data file could not be opened or created.");

        serde_yaml::to_writer(file, self).unwrap();
    }
}

/// Data about searching for text in files.
#[derive(Serialize, Deserialize, Default)]
pub struct SearcherData {
    /// The history of searches from oldest to newest.
    pub history: VecDeque<String>,
}

impl SearcherData {
    /// Add an entry to the history.
    pub fn add_to_history(&mut self, phrase: &str, max_length: usize) {
        self.history.push_back(phrase.to_string());
        if self.history.len() > max_length {
            self.history.pop_front();
        }
    }
}
