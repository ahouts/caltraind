use std::collections::BTreeSet;
use std::time::Duration;

use clap::{crate_authors, crate_description, crate_version, App, AppSettings, Arg, SubCommand};

use crate::caltrain_status::{Direction, TrainType};
use crate::daemon::close_existing;
use crate::station::Station;

mod caltrain_status;
pub(crate) mod cfg;
mod daemon;
mod station;

const STATION_LONG_HELP: &str =
    "caltrain station to generate notifications for\nvalid stations include: SanFrancisco, \
     TwentySecondStreet, Bayshore, SouthSanFrancisco, SanBruno, MillbraeTransitCenter, \
     Broadway, Burlingame, SanMateo, HaywardPark, Hillsdale, Belmont, SanCarlos, RedwoodCity, \
     Atherton, MenloPark, PaloAlto, CaliforniaAve, SanAntonio, MountainView, Sunnyvale, \
     Lawrence, SantaClara, CollegePark, SanJoseDiridon, Tamien, Capitol, BlossomHill, \
     MorganHill, SanMartin, Gilroy";

fn main() {
    let root_matches = App::new("caltraind")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .subcommand(SubCommand::with_name("start")
            .arg(Arg::with_name("THREADS")
                .short("T")
                .long("threads")
                .takes_value(true)
                .default_value("2")
                .help("number of worker threads for asynchronous runtime"))
            .arg(Arg::with_name("TYPES")
                .short("t")
                .long("types")
                .takes_value(true)
                .default_value("Local,Limited,BabyBullet")
                .help("train types to generate notifications for"))
            .arg(Arg::with_name("STATION")
                .short("s")
                .long("station")
                .takes_value(true)
                .default_value("PaloAlto")
                .help("caltrain station to generate notifications for [valid stations in extended help]")
                .long_help(STATION_LONG_HELP))
            .arg(Arg::with_name("DIRECTION")
                .short("d")
                .long("direction")
                .takes_value(true)
                .required(true)
                .help("generate notifications for trains heading in direction [Northbound Southbound]"))
            .arg(Arg::with_name("REFRESH_RATE")
                .short("r")
                .long("refresh-rate")
                .takes_value(true)
                .default_value("20")
                .help("how often in seconds to query caltrain for updates"))
            .arg(Arg::with_name("NOTIFY_AT")
                .short("n")
                .long("notify-at")
                .takes_value(true)
                .multiple(true)
                .required(true)
                .help("number of minutes before train departure to notify")))
        .subcommand(SubCommand::with_name("kill")
            .about("kill existing caltraind instance"))
        .setting(AppSettings::SubcommandRequired)
        .get_matches();

    if root_matches.subcommand_matches("kill").is_some() {
        close_existing();
        return;
    }

    let matches = root_matches.subcommand_matches("start").unwrap();

    let n_threads: usize = matches
        .value_of("THREADS")
        .unwrap()
        .parse()
        .expect("error while parsing number of threads");

    let train_types: BTreeSet<TrainType> = matches
        .values_of("TYPES")
        .unwrap()
        .map(|t| t.split_terminator(","))
        .flatten()
        .map(|t| serde_yaml::from_str(t).expect("error parsing train type"))
        .collect();

    let station: Station =
        serde_yaml::from_str(matches.value_of("STATION").unwrap()).expect("error parsing station");

    let direction: Direction = serde_yaml::from_str(matches.value_of("DIRECTION").unwrap())
        .expect("error parsing direction");

    let refresh_rate = Duration::from_secs(
        matches
            .value_of("REFRESH_RATE")
            .unwrap()
            .parse()
            .expect("error parsing refresh rate"),
    );

    let notify_at: Vec<u16> = matches
        .values_of("NOTIFY_AT")
        .unwrap()
        .map(|n| n.parse().expect("invalid notification time"))
        .collect();

    daemon::start(
        n_threads,
        train_types,
        station,
        direction,
        refresh_rate,
        notify_at,
    )
    .unwrap();
}
