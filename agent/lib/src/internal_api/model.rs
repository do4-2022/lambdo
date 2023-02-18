use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use unshare;

#[derive(Deserialize, Serialize, Debug)]
pub struct FileModel {
    pub path: PathBuf,
    pub file_name: String,
    pub content: String,

}

impl FileModel {
    pub fn new(path: PathBuf, file_name: String, content: String) -> Self {
        Self {
            path,
            file_name,
            content,
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CodeReturn {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl CodeReturn {
    pub fn new(stdout: String, stderr: String, exit_code: i32) -> Self {
        Self {
            stdout,
            stderr,
            exit_code,
        }
    }
}

#[derive(Debug)]
pub enum InternalError {
    CmdSpawn(unshare::Error),
    ChildWait(std::io::Error),
    ChildExitError(i32),
    InvalidExitCode,
    StdoutRead,
}
