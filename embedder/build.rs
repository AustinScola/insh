//! The build script for the embedder.
//!
//! The model is downloaded here rather than at run time so that the executable carries it and
//! nothing has to be fetched the first time inshd starts. Set `EMBEDDER_MODEL_DIR` to the path of
//! a directory holding the files to build without a network.
#![allow(clippy::needless_return)]

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The repository which the model comes from.
const REPOSITORY: &str = "minishlab/potion-base-8M";

/// The revision of the model which is embedded.
const REVISION: &str = "bf8b056651a2c21b8d2565580b8569da283cab23";

/// The files of the model and the checksums which they have to have.
const FILES: [(&str, &str); 3] = [
    (
        "model.safetensors",
        "f65d0f325faadc1e121c319e2faa41170d3fa07d8c89abd48ca5358d9a223de2",
    ),
    (
        "tokenizer.json",
        "e67e803f624fb4d67dea1c730d06e1067e1b14d830e2c2202569e3ef0f70bb50",
    ),
    (
        "config.json",
        "2a6ac0e9aaa356a68a5688070db78fc3a464fefe85d2f06a1905ce3718687553",
    ),
];

/// The environment variable which says where to read the model from instead of downloading it.
const MODEL_DIR_VAR: &str = "EMBEDDER_MODEL_DIR";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={}", MODEL_DIR_VAR);

    let out_dir: PathBuf = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let model_dir: Option<PathBuf> = env::var(MODEL_DIR_VAR).ok().map(PathBuf::from);

    for (name, checksum) in FILES {
        let destination: PathBuf = out_dir.join(name);

        // A rebuild after a change to something else should not download the model again.
        if let Ok(bytes) = fs::read(&destination) {
            if digest(&bytes) == checksum {
                continue;
            }
        }

        let bytes: Vec<u8> = match &model_dir {
            Some(model_dir) => read(&model_dir.join(name)),
            None => download(name),
        };

        let actual: String = digest(&bytes);
        if actual != checksum {
            panic!(
                "The checksum of {} is {} but it should be {}.",
                name, actual, checksum
            );
        }

        fs::write(&destination, &bytes)
            .unwrap_or_else(|error| panic!("Failed to write {:?}: {}", destination, error));
    }
}

/// Return the SHA-256 checksum of some bytes.
fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    return format!("{:x}", hasher.finalize());
}

/// Return the contents of a file of the model.
fn read(path: &Path) -> Vec<u8> {
    return fs::read(path).unwrap_or_else(|error| {
        panic!(
            "Failed to read {:?}, which {} points at: {}",
            path, MODEL_DIR_VAR, error
        )
    });
}

/// Return a file of the model, downloaded.
fn download(name: &str) -> Vec<u8> {
    let url: String = format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        REPOSITORY, REVISION, name
    );

    let mut response = ureq::get(&url).call().unwrap_or_else(|error| {
        panic!(
            "Failed to download {}: {}. Set {} to build without a network.",
            url, error, MODEL_DIR_VAR
        )
    });

    let mut bytes: Vec<u8> = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .unwrap_or_else(|error| panic!("Failed to read {}: {}", url, error));

    return bytes;
}
