/*!
This module contains the [`Psql`] struct which runs the PostgreSQL interactive terminal against the
database.

The terminal is one of the programs which are extracted alongside the server, so it does not have to
be installed separately either.
*/
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use crate::database::{DATABASE_NAME, POSTGRES_PORT};

use common::paths::INSHD_POSTGRES_DIR;

use postgresql_commands::psql::PsqlBuilder;
use postgresql_commands::CommandBuilder;
use postgresql_embedded::BOOTSTRAP_SUPERUSER;

/// The name of the directory that the PostgreSQL programs are installed in.
const BIN_DIR_NAME: &str = "bin";

/// The name of the PostgreSQL interactive terminal program.
const PSQL_PROGRAM_NAME: &str = "psql";

/// The PostgreSQL interactive terminal.
pub struct Psql {}

impl Psql {
    /// Run the PostgreSQL interactive terminal against the database.
    ///
    /// The version of the server which is running has to be given because the programs are
    /// installed in a directory which is named after their version.
    ///
    /// If a command is given, then it is run and the terminal exits instead of prompting.
    pub fn run(version: &str, command: Option<&str>) -> Result<ExitStatus, RunError> {
        let bin_dir: PathBuf = INSHD_POSTGRES_DIR.join(version).join(BIN_DIR_NAME);

        let program: PathBuf = bin_dir.join(PSQL_PROGRAM_NAME);
        if !program.exists() {
            return Err(RunError::NotInstalled(program));
        }

        let mut builder: PsqlBuilder = PsqlBuilder::new()
            .program_dir(bin_dir)
            // PostgreSQL takes the directory that the unix socket is in as the host.
            .host(&*INSHD_POSTGRES_DIR)
            .port(POSTGRES_PORT)
            .username(BOOTSTRAP_SUPERUSER)
            .dbname(DATABASE_NAME)
            // Connections are authenticated by operating system user, so there is no password to
            // prompt for.
            .no_password();
        if let Some(command) = command {
            builder = builder.command(command);
        }

        // The terminal reads from and writes to the terminal of this process.
        let mut psql: Command = builder.build();
        return psql.status().map_err(RunError::RunFailed);
    }
}

mod run_error {
    //! An error running the PostgreSQL interactive terminal.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;
    use std::path::PathBuf;

    /// An error running the PostgreSQL interactive terminal.
    pub enum RunError {
        /// The PostgreSQL interactive terminal is not installed.
        NotInstalled(PathBuf),
        /// An error running the PostgreSQL interactive terminal.
        RunFailed(IOError),
    }

    impl Display for RunError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::NotInstalled(path) => {
                    write!(formatter, "There is no PostgreSQL terminal at {:?}", path)
                }
                Self::RunFailed(error) => {
                    write!(
                        formatter,
                        "Failed to run the PostgreSQL terminal: {}",
                        error
                    )
                }
            }
        }
    }
}
pub use run_error::RunError;
