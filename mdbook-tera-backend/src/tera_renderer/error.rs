use std::path::StripPrefixError;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RendererError {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Error rendering tera template: {0}")]
    TeraError(#[from] tera::Error),
    #[error("HTML Error: {0}")]
    HtmlRewritingError(#[from] lol_html::errors::RewritingError),
    #[error("Mdbook Error: {0}")]
    Mdbook(#[from] mdbook::errors::Error),
    #[error("Error in strip_prefix call: {0}")]
    StripPrefixError(#[from] StripPrefixError),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Serde error: {0}")]
    DependencyNotFound(String),
}

pub type Result<T> = std::result::Result<T, RendererError>;
