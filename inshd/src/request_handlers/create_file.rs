//! Handles requests to create a file.
use std::fs::{DirBuilder, File};
use std::path::PathBuf;

use file_type::FileType;
use insh_api::{
    CreateFileError, CreateFileRequestParams, CreateFileResponseParams, CreateFileResult,
    ResponseParams, ResponseParamsAndLast,
};

/// Handles creating a file.
pub struct CreateFile {
    /// The path of the file to create.
    path: PathBuf,
    /// The type of file to create.
    file_type: FileType,
    /// Whether or not created the file is done.
    done: bool,
}

impl CreateFile {
    /// Return a file creator.
    pub fn new(params: &CreateFileRequestParams) -> Self {
        Self {
            path: params.path().to_path_buf(),
            file_type: params.file_type(),
            done: false,
        }
    }
}

impl Iterator for CreateFile {
    type Item = ResponseParamsAndLast;

    fn next(&mut self) -> Option<ResponseParamsAndLast> {
        if self.done {
            return None;
        }

        let create_file_result: CreateFileResult = if self.path.exists() {
            Err(CreateFileError::AlreadyExists(self.path.clone()))
        } else {
            match self.file_type {
                FileType::File => {
                    log::info!("Creating file {:?}...", self.path);
                    match File::create(&self.path) {
                        Ok(_) => {
                            log::info!("Created file {:?}.", self.path);
                            Ok(())
                        }
                        Err(io_error) => {
                            log::error!("Error creating file: {}", io_error);
                            Err(CreateFileError::Other(format!("{}", io_error)))
                        }
                    }
                }
                FileType::Dir => {
                    log::info!("Creating directory {:?}...", self.path);
                    match DirBuilder::new().create(&self.path) {
                        Ok(_) => {
                            log::info!("Created directory {:?}.", self.path);
                            Ok(())
                        }
                        Err(io_error) => {
                            log::error!("Error creating directory: {}", io_error);
                            Err(CreateFileError::Other(format!("{}", io_error)))
                        }
                    }
                }
                file_type => Err(CreateFileError::UnsupportedFileType(file_type)),
            }
        };
        let response_params: ResponseParams = ResponseParams::CreateFile(
            CreateFileResponseParams::builder()
                .result(create_file_result)
                .build(),
        );

        self.done = true;

        Some(
            ResponseParamsAndLast::builder()
                .response_params(response_params)
                .last(true)
                .build(),
        )
    }
}
