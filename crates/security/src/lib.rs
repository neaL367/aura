#![allow(dead_code)]

pub mod path_safety;
pub mod pipe_security;
pub mod process_security;
pub mod redact;

pub use path_safety::{
    PathError, check_symlink_depth, get_allowed_directories, is_symlink, validate_path,
};
pub use pipe_security::{SecurityDescriptor, FILE_GENERIC_READ, FILE_GENERIC_WRITE};
pub use process_security::ClientValidator;
pub use process_security::validate_client_pid;
pub use redact::redact_path;

pub use pipe_security::get_named_pipe_client_pid;
