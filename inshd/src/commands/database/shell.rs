//! Opens a shell for the database of inshd.
use std::process::ExitStatus;

use insh_api::{
    DatabaseInfoRequestParams, DatabaseInfoResponseParams, Request, RequestParams, Response,
    ResponseParams,
};
use insh_db::Psql;
use inshd_client::InshdClient;

/// Opens a shell for the database of inshd.
pub struct DatabaseShell {}

impl DatabaseShell {
    /// Open a shell for the database of inshd.
    ///
    /// The version of the database server is needed to find the shell program, and inshd is the
    /// only thing which knows which version is running, so ask it.
    pub fn run(options: &DatabaseShellOptions) -> Result<i32, DatabaseShellError> {
        let mut client: InshdClient = match InshdClient::connect() {
            Ok(client) => client,
            Err(error) => {
                return Err(DatabaseShellError::FailedToConnect(error));
            }
        };

        // Ask inshd about the database.
        let request: Request = Request::builder()
            .params(RequestParams::DatabaseInfo(
                DatabaseInfoRequestParams::builder().build(),
            ))
            .build();
        if let Err(error) = client.send(&request) {
            return Err(DatabaseShellError::FailedToSendRequest(error));
        }

        let response: Response = match client.receive() {
            Ok(response) => response,
            Err(error) => {
                return Err(DatabaseShellError::FailedToReceiveResponse(error));
            }
        };
        let params: &DatabaseInfoResponseParams = match response.params() {
            ResponseParams::DatabaseInfo(params) => params,
            _ => {
                return Err(DatabaseShellError::UnexpectedResponse);
            }
        };
        let version: String = params.version().to_string();

        // Disconnect so that inshd does not keep a client handler around for as long as the shell
        // is open.
        drop(client);

        let exit_status: ExitStatus = match Psql::run(&version, options.command.as_deref()) {
            Ok(exit_status) => exit_status,
            Err(error) => {
                return Err(DatabaseShellError::FailedToRunShell(error));
            }
        };

        return Ok(exit_status.code().unwrap_or(1));
    }
}

mod database_shell_options {
    //! Options for opening a shell for the database of inshd.

    use crate::args::ShellArgs;

    /// Options for opening a shell for the database of inshd.
    pub struct DatabaseShellOptions {
        /// A command to run instead of prompting.
        pub command: Option<String>,
    }

    impl DatabaseShellOptions {
        /// Return new database shell options.
        pub fn new(shell_args: &ShellArgs) -> Self {
            Self {
                command: shell_args.command.clone(),
            }
        }
    }
}
pub use database_shell_options::DatabaseShellOptions;

mod database_shell_error {
    //! An error opening a shell for the database of inshd.

    use std::fmt::{Display, Error as FmtError, Formatter};

    use insh_db::PsqlRunError;
    use inshd_client::{ConnectError, ReceiveError, SendError};

    /// An error opening a shell for the database of inshd.
    pub enum DatabaseShellError {
        /// Failed to connect to inshd.
        FailedToConnect(ConnectError),
        /// Failed to send the request for information about the database.
        FailedToSendRequest(SendError),
        /// Failed to receive a response.
        FailedToReceiveResponse(ReceiveError),
        /// Inshd responded with something other than information about the database.
        UnexpectedResponse,
        /// Failed to run the shell.
        FailedToRunShell(PsqlRunError),
    }

    impl Display for DatabaseShellError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::FailedToConnect(error) => {
                    write!(formatter, "{}", error)
                }
                Self::FailedToSendRequest(error) => {
                    write!(
                        formatter,
                        "Failed to request information about the database: {}",
                        error
                    )
                }
                Self::FailedToReceiveResponse(error) => {
                    write!(formatter, "{}", error)
                }
                Self::UnexpectedResponse => {
                    write!(formatter, "Inshd sent an unexpected response.")
                }
                Self::FailedToRunShell(error) => {
                    write!(formatter, "{}", error)
                }
            }
        }
    }
}
use database_shell_error::DatabaseShellError;
