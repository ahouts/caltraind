use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{read_to_string, File};
use std::time::Duration;

use actix::{Actor, System};
use actix_web::{App, HttpServer};
use daemonize::Daemonize;
use nix::errno::Errno;
use nix::sys::signal;
use nix::unistd::Pid;

use crate::caltrain_status::{Direction, TrainType};
use crate::cfg::{CALTRAIND_PATH, PID_PATH, SOCKET_PATH, STDERR_PATH, STDOUT_PATH};
use crate::daemon::cstatus_fetcher::CStatusFetcher;
use crate::daemon::notifier::Notifier;
use crate::station::Station;

mod cstatus_fetcher;
mod notifier;

pub fn close_existing() {
    let pid = match read_to_string(PID_PATH.as_path()) {
        Ok(s) => s.parse::<i32>().expect("pid file did not contain pid"),
        Err(_) => return,
    };
    match signal::kill(Pid::from_raw(pid), signal::SIGTERM) {
        Ok(_) | Err(nix::Error::Sys(Errno::ESRCH)) => {
            std::fs::remove_file(PID_PATH.as_path()).expect("error deleting pid file")
        }
        _ => (),
    }
}

fn daemonize() -> Result<(), Box<dyn Error>> {
    close_existing();
    Daemonize::new()
        .pid_file(PID_PATH.as_path())
        .chown_pid_file(true)
        .working_directory(CALTRAIND_PATH.as_path())
        .stdout(File::create(STDOUT_PATH.as_path())?)
        .stderr(File::create(STDERR_PATH.as_path())?)
        .start()?;
    Ok(())
}

pub fn start(
    n_threads: usize,
    train_types: BTreeSet<TrainType>,
    station: Station,
    direction: Direction,
    refresh_rate: Duration,
    notify_at: Vec<u16>,
) -> Result<(), Box<dyn Error>> {
    let sys = System::new("caltraind");

    daemonize()?;

    CStatusFetcher::new(station, refresh_rate).start();
    for n in notify_at {
        Notifier::new(train_types.clone(), n, direction).start();
    }

    HttpServer::new(|| App::new())
        .workers(n_threads)
        .bind_uds(SOCKET_PATH.as_path())?
        .start();

    sys.run()?;

    Ok(())
}
