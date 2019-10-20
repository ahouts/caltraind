use std::fmt;
use std::io::Read;

use actix::prelude::*;
use html5ever::{
    parse_document,
    rcdom::{Handle, NodeData, RcDom},
    tendril::TendrilSink,
};
use once_cell::sync::Lazy;
use regex::Regex;
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

    pub fn from_html<R: Read>(mut text: R) -> Result<CaltrainStatus, Error> {
        let mut train_id = None;
        let mut train_type = None;
        let mut time_till_arrival = None;
        let mut last_text = None;
        let mut current_table_no = 0;
        let mut southbound = vec![];
        let mut northbound = vec![];

        #[derive(Clone, Copy)]
        enum LastReadClass {
            TrainId,
            TrainType,
            TimeTillArrival,
        }

        use LastReadClass::*;

        let mut last_read_class = None;

        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut text)?;

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

        fn walk(
            handle: &Handle,
            last_read_class: &mut Option<LastReadClass>,
            last_text: &mut Option<String>,
            mut train_id: &mut Option<String>,
            mut train_type: &mut Option<String>,
            mut time_till_arrival: &mut Option<String>,
            current_table_no: &mut i32,
            southbound: &mut Vec<IncomingTrain>,
            northbound: &mut Vec<IncomingTrain>,
        ) -> Result<(), Error> {
            let node = handle;

            match node.data {
                NodeData::Element { ref attrs, .. } => {
                    for attr in attrs.borrow().iter() {
                        if &attr.name.local == "class" {
                            let val = attr.value.as_bytes();
                            if val.ends_with(b"ipf-st-ip-trains-subtable") {
                                *current_table_no += 1;
                            }
                            if val.ends_with(b"ipf-st-ip-trains-subtable-td-id") {
                                *last_read_class = Some(TrainId);
                            }
                            if val.ends_with(b"ipf-st-ip-trains-subtable-td-type") {
                                *last_read_class = Some(TrainType);
                            }
                            if val.ends_with(b"ipf-st-ip-trains-subtable-td-arrivaltime") {
                                *last_read_class = Some(TimeTillArrival);
                            }
                        }
                    }
                }
                NodeData::Text { ref contents } => {
                    *last_text = Some(contents.borrow().to_string());
                }
                _ => (),
            }
            for child in node.children.borrow().iter() {
                walk(
                    child,
                    last_read_class,
                    last_text,
                    train_id,
                    train_type,
                    time_till_arrival,
                    current_table_no,
                    southbound,
                    northbound,
                )?;
            }
            let res = match (&last_read_class, &last_text) {
                (Some(ttype), Some(text)) => {
                    match ttype {
                        TrainId => *train_id = Some(text.clone()),
                        TrainType => *train_type = Some(text.clone()),
                        TimeTillArrival => *time_till_arrival = Some(text.clone()),
                    }
                    (None, None)
                }
                (a, b) => (**a, (b.as_ref().map(|s| s.clone())).clone()),
            };
            *last_read_class = res.0;
            *last_text = res.1;
            let mut should_wipe = false;
            match (&mut train_id, &mut train_type, &mut time_till_arrival) {
                (Some(tid), Some(ttype), Some(tta)) => {
                    if *current_table_no == 1 {
                        southbound.push(make_incoming_train(tid, ttype, tta)?);
                    }
                    if *current_table_no == 2 {
                        northbound.push(make_incoming_train(tid, ttype, tta)?);
                    }
                    should_wipe = true;
                }
                _ => (),
            }
            if should_wipe {
                *train_id = None;
                *train_type = None;
                *time_till_arrival = None;
            }
            Ok(())
        }

        walk(
            &dom.document,
            &mut last_read_class,
            &mut last_text,
            &mut train_id,
            &mut train_type,
            &mut time_till_arrival,
            &mut current_table_no,
            &mut southbound,
            &mut northbound,
        )?;

        Ok(CaltrainStatus {
            northbound,
            southbound,
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
    use std::io::Cursor;

    use super::*;

    #[test]
    fn from_html() {
        assert_eq!(
            CaltrainStatus::from_html(Cursor::new(include_str!("test.html"))).unwrap(),
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
            CaltrainStatus::from_html(Cursor::new(include_str!("test2.html"))).unwrap(),
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
