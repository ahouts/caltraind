use std::fmt;

use actix::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};

use crate::caltrain_status::Error::{HtmlError, InvalidIntError};

static NUMERIC: Lazy<Regex> = Lazy::new(|| Regex::new("[0-9]+").unwrap());

#[derive(Serialize, Deserialize, Clone, Copy, PartialOrd, Ord, Eq, PartialEq, Debug)]
pub enum TrainType {
    Local,
    Limited,
    BabyBullet,
}

impl<T: AsRef<str>> From<T> for TrainType {
    fn from(s: T) -> Self {
        if s.as_ref().contains("Local") {
            TrainType::Local
        } else if s.as_ref().contains("Limited") {
            TrainType::Limited
        } else if s.as_ref().contains("Baby Bullet") {
            TrainType::BabyBullet
        } else {
            panic!("error, unknown train type: {}", s.as_ref());
        }
    }
}

impl fmt::Display for TrainType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TrainType::*;
        match self {
            Local => write!(f, "Local"),
            Limited => write!(f, "Limited"),
            BabyBullet => write!(f, "Baby Bullet"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct IncomingTrain {
    id: u16,
    ttype: TrainType,
    min_till_departure: u16,
}

impl IncomingTrain {
    fn new(id: u16, ttype: TrainType, min_till_arrival: u16) -> Self {
        IncomingTrain {
            id,
            ttype,
            min_till_departure: min_till_arrival,
        }
    }

    pub fn get_id(&self) -> u16 {
        self.id
    }

    pub fn get_train_type(&self) -> TrainType {
        self.ttype
    }

    pub fn get_min_till_departure(&self) -> u16 {
        self.min_till_departure
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialOrd, Ord, Eq, PartialEq, Debug)]
pub enum Direction {
    Northbound,
    Southbound,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CaltrainStatus {
    northbound: Vec<IncomingTrain>,
    southbound: Vec<IncomingTrain>,
}

impl CaltrainStatus {
    pub fn get_status(&self) -> (&[IncomingTrain], &[IncomingTrain]) {
        (self.northbound.as_ref(), self.southbound.as_ref())
    }

    pub fn from_html<T: AsRef<str>>(text: T) -> Result<CaltrainStatus, Error> {
        struct WalkerState {
            train_id: Option<String>,
            train_type: Option<String>,
            time_till_departure: Option<String>,
            last_text: Option<String>,
            current_table_no: i32,
            last_read_class: Option<LastReadClass>,
            northbound: Vec<IncomingTrain>,
            southbound: Vec<IncomingTrain>,
        }

        let mut state = WalkerState {
            train_id: None,
            train_type: None,
            time_till_departure: None,
            last_text: None,
            current_table_no: 0,
            last_read_class: None,
            southbound: vec![],
            northbound: vec![],
        };

        #[derive(Clone, Copy)]
        enum LastReadClass {
            TrainId,
            TrainType,
            TimeTillArrival,
        }

        use LastReadClass::*;

        let dom = Html::parse_document(text.as_ref());

        fn make_incoming_train(tid: &str, ttype: &str, tta: &str) -> Result<IncomingTrain, Error> {
            let tid = tid.parse::<u16>()?;
            let ttype = ttype.into();
            let min_till_arrival = if let Some(m) = NUMERIC.find(&tta) {
                m.as_str().parse::<u16>()?
            } else {
                9001
            };
            Ok(IncomingTrain::new(tid, ttype, min_till_arrival))
        }

        fn walk(node: &ElementRef, state: &mut WalkerState) -> Result<(), Error> {
            for attr in &node.value().attrs {
                if &attr.0.local == "class" {
                    let val = attr.1.as_bytes();
                    if val.ends_with(b"ipf-st-ip-trains-subtable") {
                        state.current_table_no += 1;
                    }
                    if val.ends_with(b"ipf-st-ip-trains-subtable-td-id") {
                        state.last_read_class = Some(TrainId);
                    }
                    if val.ends_with(b"ipf-st-ip-trains-subtable-td-type") {
                        state.last_read_class = Some(TrainType);
                    }
                    if val.ends_with(b"ipf-st-ip-trains-subtable-td-arrivaltime") {
                        state.last_read_class = Some(TimeTillArrival);
                    }
                }
            }

            for child in node.children() {
                if let Some(e) = ElementRef::wrap(child) {
                    walk(&e, state)?;
                } else {
                    if let Some(t) = child.value().as_text() {
                        state.last_text = Some(t.text.to_string());
                    }
                }
            }
            let res = match (&state.last_read_class, &state.last_text) {
                (Some(ttype), Some(text)) => {
                    match ttype {
                        TrainId => state.train_id = Some(text.clone()),
                        TrainType => state.train_type = Some(text.clone()),
                        TimeTillArrival => state.time_till_departure = Some(text.clone()),
                    }
                    (None, None)
                }
                (a, b) => (*a, b.as_ref().cloned()),
            };
            state.last_read_class = res.0;
            state.last_text = res.1;
            let mut should_wipe = false;
            if let (Some(tid), Some(ttype), Some(tta)) = (
                &mut state.train_id,
                &mut state.train_type,
                &mut state.time_till_departure,
            ) {
                if state.current_table_no == 1 {
                    state.southbound.push(make_incoming_train(tid, ttype, tta)?);
                }
                if state.current_table_no == 2 {
                    state.northbound.push(make_incoming_train(tid, ttype, tta)?);
                }
                should_wipe = true;
            }

            if should_wipe {
                state.train_id = None;
                state.train_type = None;
                state.time_till_departure = None;
            }
            Ok(())
        }

        walk(&dom.root_element(), &mut state)?;

        Ok(CaltrainStatus {
            northbound: state.northbound,
            southbound: state.southbound,
        })
    }
}

impl Message for CaltrainStatus {
    type Result = ();
}

#[derive(Debug)]
pub enum Error {
    HtmlError(std::io::Error),
    InvalidIntError(std::num::ParseIntError),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HtmlError(e) => write!(f, "{:?}", e),
            InvalidIntError(e) => write!(f, "{}", e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        HtmlError(e)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Self {
        InvalidIntError(e)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_html() {
        assert_eq!(
            CaltrainStatus::from_html(include_str!("test.html")).unwrap(),
            CaltrainStatus {
                northbound: vec![
                    IncomingTrain {
                        id: 429,
                        ttype: TrainType::Local,
                        min_till_departure: 59,
                    },
                    IncomingTrain {
                        id: 431,
                        ttype: TrainType::Local,
                        min_till_departure: 149,
                    },
                    IncomingTrain {
                        id: 433,
                        ttype: TrainType::Local,
                        min_till_departure: 239,
                    }
                ],
                southbound: vec![
                    IncomingTrain {
                        id: 802,
                        ttype: TrainType::BabyBullet,
                        min_till_departure: 6,
                    },
                    IncomingTrain {
                        id: 428,
                        ttype: TrainType::Local,
                        min_till_departure: 63,
                    },
                    IncomingTrain {
                        id: 430,
                        ttype: TrainType::Local,
                        min_till_departure: 153,
                    }
                ],
            }
        )
    }

    #[test]
    fn from_html_no_southbound() {
        assert_eq!(
            CaltrainStatus::from_html(include_str!("test2.html")).unwrap(),
            CaltrainStatus {
                northbound: vec![
                    IncomingTrain {
                        id: 803,
                        ttype: TrainType::BabyBullet,
                        min_till_departure: 69,
                    },
                    IncomingTrain {
                        id: 435,
                        ttype: TrainType::Local,
                        min_till_departure: 86,
                    },
                    IncomingTrain {
                        id: 437,
                        ttype: TrainType::Local,
                        min_till_departure: 176,
                    }
                ],
                southbound: vec![],
            }
        )
    }
}
