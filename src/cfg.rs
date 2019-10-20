use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

pub static CALTRAIND_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let p = PathBuf::from("/tmp/caltraind");
    create_dir_all(&p).expect("error creating /tmp/caltraind");
    p
});

pub static PID_PATH: Lazy<PathBuf> =
    Lazy::new(|| Path::new(CALTRAIND_PATH.as_os_str()).join("pid"));
pub static SOCKET_PATH: Lazy<PathBuf> =
    Lazy::new(|| Path::new(CALTRAIND_PATH.as_os_str()).join("socket"));
pub static STDOUT_PATH: Lazy<PathBuf> =
    Lazy::new(|| Path::new(CALTRAIND_PATH.as_os_str()).join("out.log"));
pub static STDERR_PATH: Lazy<PathBuf> =
    Lazy::new(|| Path::new(CALTRAIND_PATH.as_os_str()).join("err.log"));
