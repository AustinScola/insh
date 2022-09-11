use std::collections::VecDeque;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::ErrorKind as IOErrorKind;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

use fslock::LockFile;

use serde::{Deserialize, Serialize};

/// The permissions to use for the data directory and files in it.
static INSH_DIRECTORY_PERMISSIONS: u32 = 0o700; // rwx --- ---
static INSH_FILES_PERMISSIONS: u32 = 0o600; // rw- --- ---

lazy_static! {

    /// The directory that the data file is stored in for a user.
    static ref INSH_DIRECTORY: Option<PathBuf> = {
        let home: Option<PathBuf> = dirs::home_dir();
        match home {
            Some(home) => {
                #[allow(clippy::redundant_clone)]
                let mut path = home.clone();
                path.push(".insh");
                Some(path)
            },
            None => None,
        }
    };

    /// The file path for user data. This is `None` if the home directory of the user cannot be
    /// determined.
    static ref PATH: Option<PathBuf> = {
        let home: Option<PathBuf> = dirs::home_dir();
        match home {
            Some(home) => {
                #[allow(clippy::redundant_clone)]
                let mut path = home.clone();
                path.push(".insh/data.yaml");
                Some(path)
            },
            None => None,
        }
    };

    /// The file path for the lock file on data. This is `None` if the home directory of the user
    /// cannot be determined.
    static ref LOCK_FILE_PATH: Option<PathBuf> = {
        let home: Option<PathBuf> = dirs::home_dir();
        match home {
            Some(home) => {
                #[allow(clippy::redundant_clone)]
                let mut path = home.clone();
                path.push(".insh/data.lock");
                Some(path)
            },
            None => None,
        }
    };
}

#[derive(Serialize, Deserialize)]
pub struct Data {
    #[serde(skip, default = "get_lock_file")]
    lock: LockFile,

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
    ensure_insh_directory_exists();

    let lock_file_path: PathBuf = match &*LOCK_FILE_PATH {
        Some(path) => path.to_path_buf(),
        None => {
            panic!("Cannot get the data file lock because the home directory of the user cannot be determined.");
        }
    };

    // NOTE: The lock file is created w/ the permissions -rw-r--r--. It would be nice if we could
    // change tell it to create it w/ -rw------- but it doesn't look like it has that capability.
    // We could change the perms after it is created but this is probably fine for now.
    let mut lock_file = LockFile::open(&lock_file_path).unwrap();
    lock_file.lock_with_pid().unwrap();
    lock_file
}

/// Ensure that the Insh directory used for storing data exists.
fn ensure_insh_directory_exists() {
    match &*INSH_DIRECTORY {
        Some(insh_directory) => {
            if !insh_directory.exists() {
                DirBuilder::new()
                    .mode(INSH_DIRECTORY_PERMISSIONS)
                    .create(insh_directory)
                    .expect("Failed to create the insh directory.");
            }
        }
        None => {
            panic!("Cannot create the Insh directory because the home directory of the user cannot be determined.");
        }
    }
}

impl Data {
    #[allow(dead_code)]
    pub fn acquire(&mut self) {
        self.lock = get_lock_file();
    }

    fn has_lock(&self) -> bool {
        self.lock.owns_lock()
    }

    pub fn release(&mut self) {
        self.lock.unlock().unwrap();
    }

    /// Read the data from the file system. If the data file does not exist, then return the default
    /// data.
    ///
    /// This also aquires a lock on the data.
    pub fn read() -> Self {
        let path: PathBuf = match &*PATH {
            Some(path) => path.to_path_buf(),
            None => {
                panic!();
                // return Self::default();
            }
        };

        let file: File = match File::open(path) {
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

        let path: PathBuf = match &*PATH {
            Some(path) => path.to_path_buf(),
            None => {
                panic!("Cannot write the data because the path cannot be determined.");
            }
        };

        let file: File = OpenOptions::new()
            .write(true)
            .create(true)
            .mode(INSH_FILES_PERMISSIONS)
            .open(path)
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
