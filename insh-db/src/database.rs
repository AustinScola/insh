/*!
This module contains the [`Database`] struct which runs the embedded PostgreSQL server and provides
connections to it.

The PostgreSQL binaries are embedded in the executable, so PostgreSQL does not have to be installed
separately. They are extracted into the insh directory the first time the database is started. The
server only listens on a unix socket in the insh directory, never on a TCP port, and connections
over that socket are authenticated by operating system user rather than by password.
*/
use std::collections::HashMap;
use std::fs::{read_to_string, remove_dir_all, remove_file, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::db_conn_pool_events::DbConnPoolEventHandler;

use common::paths::{
    make_private_dir, INSHD_POSTGRES_DATA_DIR, INSHD_POSTGRES_DIR, INSH_FILES_PERMS,
};

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use nix::unistd::{getuid, Uid, User};
use postgresql_commands::pg_ctl::{Mode, PgCtlBuilder, ShutdownMode};
use postgresql_commands::{CommandBuilder, CommandExecutor};
use postgresql_embedded::blocking::PostgreSQL;
use postgresql_embedded::{Settings, Version};

/// The migrations which are embedded in the executable.
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// The name of the database that insh data is stored in.
pub(crate) const DATABASE_NAME: &str = "insh";

/// The port that the PostgreSQL server runs on.
///
/// The server does not listen on a TCP port. This is only here because PostgreSQL names the unix
/// socket file `.s.PGSQL.<port>` and there is no way to ask it for a different name: the name is
/// built into both the server and libpq, and neither has a setting for it. So a port is still
/// needed, it just has to be stable across restarts. The default is used to keep the socket path
/// recognizable.
pub(crate) const POSTGRES_PORT: u16 = 5432;

/// The name of the file that PostgreSQL writes its process ID to.
const POSTMASTER_PID_FILE_NAME: &str = "postmaster.pid";

/// The name of the file that PostgreSQL writes its start up logs to.
const START_LOG_FILE_NAME: &str = "start.log";

/// The name of the file PostgreSQL reads the password from.
const PWFILE_NAME: &str = "pwfile";

/// The name of the file PostgreSQL reads the authentication rules from.
const HBA_FILE_NAME: &str = "pg_hba.conf";

/// The name of the file that PostgreSQL reads the mapping of operating system users to database
/// users from.
const IDENT_FILE_NAME: &str = "pg_ident.conf";

/// The name of the mapping of the operating system user to the database user.
const IDENT_MAP_NAME: &str = "insh";

/// The maximum length of the path of the directory that the unix socket is created in.
///
/// Unix socket paths are limited to 108 bytes including the terminating null byte, and PostgreSQL
/// appends the socket file name to the directory, so the directory has to be shorter than that.
const MAX_SOCKET_DIR_LEN: usize = 107 - "/.s.PGSQL.".len() - 5;

/// Characters which are not allowed in the path of the directory that the unix socket is created
/// in.
///
/// PostgreSQL is started by passing options through a shell, which does not handle these
/// characters, and the socket directory is put in the connection URL using form encoding, which
/// encodes a space in a way that PostgreSQL does not decode.
const FORBIDDEN_SOCKET_DIR_CHARS: &str = "'\"\\$`;&|<>*?()[]{}!#~";

/// A pool of connections to the database.
pub type DbConnPool = Pool<ConnectionManager<PgConnection>>;

/// An embedded PostgreSQL database.
pub struct Database {
    /// The PostgreSQL server.
    ///
    /// This must not be cloned. It stops the server when it is dropped, so a dropped clone would
    /// stop the server out from under the daemon.
    postgres: PostgreSQL,

    /// A pool of connections to the database.
    conn_pool: DbConnPool,

    /// The version of the PostgreSQL server which is running.
    version: String,
}

impl Database {
    /// Start the database, creating and migrating it if needed.
    pub fn start(conn_pool_size: u32) -> Result<Self, StartError> {
        Self::check_socket_dir(&INSHD_POSTGRES_DIR)?;
        make_private_dir(&INSHD_POSTGRES_DIR).map_err(StartError::CreateDirFailed)?;

        let settings: Settings = Self::settings();

        // PostgreSQL writes the password file itself if it does not exist, but with permissions
        // that let the group and others read it, so write it first.
        Self::write_private_file(&settings.password_file, &settings.password)?;

        let mut postgres: PostgreSQL = PostgreSQL::new(settings);

        log::info!("Setting up the database...");
        postgres.setup().map_err(StartError::SetupFailed)?;
        // The server binaries are extracted with the permissions of the process, which let the
        // group in.
        make_private_dir(&postgres.settings().installation_dir)
            .map_err(StartError::CreateDirFailed)?;
        // The password is only used while creating the database. Connections are authenticated by
        // operating system user from here on, so there is no reason to leave it lying around.
        let _ = remove_file(&postgres.settings().password_file);
        Self::write_auth_config(postgres.settings())?;
        log::info!("Set up the database.");

        let version: String = Self::version_of(&postgres.settings().installation_dir)?;
        log::info!("The database server is version {}.", version);

        // The daemon may have been killed without getting the chance to stop the server.
        Self::stop_orphaned_server(postgres.settings());

        log::info!("Starting the database...");
        postgres
            .start()
            .map_err(|error| StartError::StartFailed(error, Self::read_start_log()))?;
        log::info!("Started the database.");

        if !postgres
            .database_exists(DATABASE_NAME)
            .map_err(StartError::DatabaseExistsFailed)?
        {
            log::info!("Creating the {} database...", DATABASE_NAME);
            postgres
                .create_database(DATABASE_NAME)
                .map_err(StartError::CreateDatabaseFailed)?;
            log::info!("Created the {} database.", DATABASE_NAME);
        }

        let url: String = postgres.settings().url(DATABASE_NAME);
        let manager: ConnectionManager<PgConnection> = ConnectionManager::new(url);
        let conn_pool: DbConnPool = Pool::builder()
            .max_size(conn_pool_size)
            .event_handler(Box::new(DbConnPoolEventHandler::new(conn_pool_size)))
            .build(manager)
            .map_err(StartError::CreatePoolFailed)?;

        log::info!("Running the database migrations...");
        {
            let mut connection = conn_pool.get().map_err(StartError::GetConnectionFailed)?;
            connection
                .run_pending_migrations(MIGRATIONS)
                .map_err(StartError::MigrationsFailed)?;
        }
        log::info!("Ran the database migrations.");

        return Ok(Self {
            postgres,
            conn_pool,
            version,
        });
    }

    /// Return a pool of connections to the database.
    pub fn conn_pool(&self) -> DbConnPool {
        return self.conn_pool.clone();
    }

    /// Return the version of the PostgreSQL server which is running.
    pub fn version(&self) -> &str {
        return &self.version;
    }

    /// Stop the database.
    pub fn stop(self) {
        log::info!("Stopping the database...");
        match self.postgres.stop() {
            Ok(_) => {
                log::info!("Stopped the database.");
            }
            Err(error) => {
                log::error!("Failed to stop the database: {}", error);
            }
        }
    }

    /// Return the settings to run the PostgreSQL server with.
    fn settings() -> Settings {
        let defaults: Settings = Settings::default();

        // The default settings put the data directory and the password file in temporary
        // directories which are created but never cleaned up. Every one of those is overridden
        // below, so remove them.
        let _ = remove_dir_all(&defaults.data_dir);
        let _ = remove_file(&defaults.password_file);
        if let Some(parent) = defaults.password_file.parent() {
            let _ = remove_dir_all(parent);
        }

        let mut configuration: HashMap<String, String> = HashMap::new();
        // Only listen on the unix socket. Nothing other than inshd connects to the database, and
        // the socket is in a directory which only the user can read.
        configuration.insert("listen_addresses".to_string(), "''".to_string());

        return Settings {
            // The server binaries are installed in a directory named after their version in here,
            // and the unix socket is created in here too.
            installation_dir: INSHD_POSTGRES_DIR.clone(),
            data_dir: INSHD_POSTGRES_DATA_DIR.clone(),
            socket_dir: Some(INSHD_POSTGRES_DIR.clone()),
            password_file: INSHD_POSTGRES_DIR.join(PWFILE_NAME),
            port: POSTGRES_PORT,
            // This has to be false. When it is true, dropping the server deletes the data
            // directory.
            temporary: false,
            configuration,
            ..defaults
        };
    }

    /// Return the version of the PostgreSQL server which is installed in a directory.
    ///
    /// The server binaries are installed in a directory which is named after their version, so
    /// there is nothing else to read the version from.
    fn version_of(installation_dir: &Path) -> Result<String, StartError> {
        let name: Option<String> = installation_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string());

        let version: Option<Version> = match &name {
            Some(name) => Version::parse(name).ok(),
            None => None,
        };

        return match version {
            Some(version) => Ok(version.to_string()),
            None => Err(StartError::UnknownVersion(installation_dir.to_path_buf())),
        };
    }

    /// Configure PostgreSQL to authenticate connections by operating system user.
    ///
    /// The database superuser is always named `postgres`, which is not the name of the user running
    /// the daemon, so a mapping between the two is needed as well.
    ///
    /// This is rewritten on every start so that it is still right if the user is renamed.
    fn write_auth_config(settings: &Settings) -> Result<(), StartError> {
        let uid: Uid = getuid();
        let user: User = User::from_uid(uid)
            .map_err(StartError::LookUpUserFailed)?
            .ok_or(StartError::NoSuchUser(uid.as_raw()))?;

        let hba: String = format!(
            "# Only connections over the unix socket are allowed, and they are authenticated by\n\
             # operating system user. This file is rewritten by inshd on every start.\n\
             local all all peer map={}\n",
            IDENT_MAP_NAME
        );
        Self::write_private_file(&settings.data_dir.join(HBA_FILE_NAME), &hba)?;

        let ident: String = format!(
            "# MAPNAME SYSTEM-USERNAME PG-USERNAME\n\
             # This file is rewritten by inshd on every start.\n\
             {} {} {}\n",
            IDENT_MAP_NAME, user.name, settings.username
        );
        Self::write_private_file(&settings.data_dir.join(IDENT_FILE_NAME), &ident)?;

        return Ok(());
    }

    /// Write a file which only the user can read.
    fn write_private_file(path: &Path, contents: &str) -> Result<(), StartError> {
        let mut file: File = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(INSH_FILES_PERMS)
            .open(path)
            .map_err(StartError::WriteFileFailed)?;
        file.write_all(contents.as_bytes())
            .map_err(StartError::WriteFileFailed)?;

        return Ok(());
    }

    /// Check that PostgreSQL is able to create a unix socket in a directory.
    fn check_socket_dir(socket_dir: &Path) -> Result<(), StartError> {
        let socket_dir_str: &str = match socket_dir.to_str() {
            Some(socket_dir_str) => socket_dir_str,
            None => {
                return Err(StartError::SocketDirNotUnicode(socket_dir.to_path_buf()));
            }
        };

        if socket_dir_str.len() > MAX_SOCKET_DIR_LEN {
            return Err(StartError::SocketDirTooLong(socket_dir.to_path_buf()));
        }

        let bad_char: Option<char> = socket_dir_str.chars().find(|character| {
            return character.is_whitespace() || FORBIDDEN_SOCKET_DIR_CHARS.contains(*character);
        });
        if let Some(bad_char) = bad_char {
            return Err(StartError::SocketDirBadChar(
                socket_dir.to_path_buf(),
                bad_char,
            ));
        }

        return Ok(());
    }

    /// Stop a PostgreSQL server which was left running for a data directory.
    ///
    /// This fails when the process ID file was left behind by a server which is no longer running.
    /// PostgreSQL removes the file itself in that case, so there is nothing else to do.
    fn stop_orphaned_server(settings: &Settings) {
        if !settings.data_dir.join(POSTMASTER_PID_FILE_NAME).exists() {
            return;
        }

        log::warn!("A database server was left running. Stopping it...");

        let mut command = PgCtlBuilder::from(settings)
            .mode(Mode::Stop)
            .pgdata(&settings.data_dir)
            .shutdown_mode(ShutdownMode::Immediate)
            .wait()
            .build();

        match command.execute() {
            Ok(_) => {
                log::info!("Stopped the database server which was left running.");
            }
            Err(error) => {
                log::debug!("Could not stop a database server: {}", error);
            }
        }
    }

    /// Return the logs that PostgreSQL wrote while starting up, if they can be read.
    ///
    /// PostgreSQL reports why it could not start in this file rather than on stderr.
    fn read_start_log() -> Option<String> {
        let path: PathBuf = INSHD_POSTGRES_DATA_DIR.join(START_LOG_FILE_NAME);
        return read_to_string(path).ok();
    }
}

mod start_error {
    //! An error starting the database.

    use std::error::Error;
    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;
    use std::path::PathBuf;

    use diesel::r2d2::PoolError;
    use nix::errno::Errno;
    use postgresql_embedded::Error as PostgresError;

    /// An error starting the database.
    pub enum StartError {
        /// The path of the unix socket directory is not valid unicode.
        SocketDirNotUnicode(PathBuf),
        /// The path of the directory to create the unix socket in is too long.
        SocketDirTooLong(PathBuf),
        /// The path of the directory to create the unix socket in contains a character which
        /// PostgreSQL cannot be told about.
        SocketDirBadChar(PathBuf, char),
        /// An error creating a directory for the database.
        CreateDirFailed(IOError),
        /// An error writing a file for the database.
        WriteFileFailed(IOError),
        /// An error looking up the user that the database is authenticated as.
        LookUpUserFailed(Errno),
        /// There is no user with the user ID that the daemon is running as.
        NoSuchUser(u32),
        /// An error installing the database server.
        SetupFailed(PostgresError),
        /// The directory the server is installed in is not named after a version.
        UnknownVersion(PathBuf),
        /// An error starting the database server, along with the start up logs if they could be
        /// read.
        StartFailed(PostgresError, Option<String>),
        /// An error determining if the database exists.
        DatabaseExistsFailed(PostgresError),
        /// An error creating the database.
        CreateDatabaseFailed(PostgresError),
        /// An error creating the pool of connections to the database.
        CreatePoolFailed(PoolError),
        /// An error getting a connection to the database from the pool.
        GetConnectionFailed(PoolError),
        /// An error running the database migrations.
        MigrationsFailed(Box<dyn Error + Send + Sync>),
    }

    impl Display for StartError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::SocketDirNotUnicode(path) => {
                    write!(
                        formatter,
                        "The database socket directory {:?} is not valid unicode",
                        path
                    )
                }
                Self::SocketDirTooLong(path) => {
                    write!(
                        formatter,
                        "The database socket directory {:?} is longer than {} bytes",
                        path,
                        super::MAX_SOCKET_DIR_LEN
                    )
                }
                Self::SocketDirBadChar(path, character) => {
                    write!(
                        formatter,
                        "The database socket directory {:?} contains {:?}",
                        path, character
                    )
                }
                Self::CreateDirFailed(error) => {
                    write!(
                        formatter,
                        "Failed to create a database directory: {}",
                        error
                    )
                }
                Self::WriteFileFailed(error) => {
                    write!(formatter, "Failed to write a database file: {}", error)
                }
                Self::LookUpUserFailed(error) => {
                    write!(formatter, "Failed to look up the user: {}", error)
                }
                Self::NoSuchUser(uid) => {
                    write!(formatter, "There is no user with the user ID {}", uid)
                }
                Self::SetupFailed(error) => {
                    write!(formatter, "Failed to install the database: {}", error)
                }
                Self::UnknownVersion(path) => {
                    write!(
                        formatter,
                        "The database is installed in {:?}, which is not named after a version",
                        path
                    )
                }
                Self::StartFailed(error, start_log) => {
                    write!(formatter, "Failed to start the database server: {}", error)?;
                    if let Some(start_log) = start_log {
                        write!(formatter, "\n{}", start_log.trim_end())?;
                    }
                    Ok(())
                }
                Self::DatabaseExistsFailed(error) => {
                    write!(
                        formatter,
                        "Failed to determine if the database exists: {}",
                        error
                    )
                }
                Self::CreateDatabaseFailed(error) => {
                    write!(formatter, "Failed to create the database: {}", error)
                }
                Self::CreatePoolFailed(error) => {
                    write!(
                        formatter,
                        "Failed to create the database connection pool: {}",
                        error
                    )
                }
                Self::GetConnectionFailed(error) => {
                    write!(
                        formatter,
                        "Failed to get a connection to the database: {}",
                        error
                    )
                }
                Self::MigrationsFailed(error) => {
                    write!(
                        formatter,
                        "Failed to run the database migrations: {}",
                        error
                    )
                }
            }
        }
    }
}
pub use start_error::StartError;

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case("/home/user/.insh/daemon/postgres/socket"; "a typical path")]
    #[test_case("/a"; "a short path")]
    fn test_check_socket_dir_allows(path: &str) {
        assert!(Database::check_socket_dir(Path::new(path)).is_ok());
    }

    #[test_case("/home/a person/.insh/socket"; "a space")]
    #[test_case("/home/user$x/.insh/socket"; "a dollar sign")]
    #[test_case("/home/user/.insh/socket;rm"; "a semicolon")]
    #[test_case("/home/o'brien/.insh/socket"; "a quote")]
    fn test_check_socket_dir_rejects_bad_chars(path: &str) {
        assert!(matches!(
            Database::check_socket_dir(Path::new(path)),
            Err(StartError::SocketDirBadChar(_, _))
        ));
    }

    #[test_case("/home/user/.insh/daemon/postgres/18.6.0", "18.6.0"; "a version")]
    #[test_case("/home/user/.insh/daemon/postgres/18.6.0/", "18.6.0"; "a trailing slash")]
    fn test_version_of(installation_dir: &str, version: &str) {
        match Database::version_of(Path::new(installation_dir)) {
            Ok(actual) => assert_eq!(actual, version),
            Err(_) => panic!("Failed to determine the version."),
        }
    }

    #[test_case("/home/user/.insh/daemon/postgres"; "not a version")]
    #[test_case("/home/user/.insh/daemon/postgres/18.6"; "a partial version")]
    #[test_case("/"; "the root directory")]
    fn test_version_of_rejects(installation_dir: &str) {
        assert!(matches!(
            Database::version_of(Path::new(installation_dir)),
            Err(StartError::UnknownVersion(_))
        ));
    }

    #[test]
    fn test_check_socket_dir_rejects_long_paths() {
        let path: String = format!("/{}", "a".repeat(MAX_SOCKET_DIR_LEN));
        assert!(matches!(
            Database::check_socket_dir(Path::new(&path)),
            Err(StartError::SocketDirTooLong(_))
        ));
    }
}
