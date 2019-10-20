use std::collections::BTreeSet;

use actix::prelude::*;
use actix_broker::BrokerSubscribe;
use chrono::{Local, NaiveTime};
use notify_rust::{Notification, Timeout};
use time::Duration;

use crate::caltrain_status::Direction::Northbound;
use crate::caltrain_status::{CaltrainStatus, Direction, TrainType};

pub struct Notifier {
    trains_notified: BTreeSet<u16>,
    notify_at: u16,
    notify_types: BTreeSet<TrainType>,
    direction: Direction,
    notify_after: Option<NaiveTime>,
}

impl Notifier {
    pub fn new(
        notify_types: BTreeSet<TrainType>,
        notify_at: u16,
        direction: Direction,
        notify_after: Option<NaiveTime>,
    ) -> Self {
        Notifier {
            notify_at,
            notify_types,
            trains_notified: BTreeSet::new(),
            direction,
            notify_after,
        }
    }
}

impl Actor for Notifier {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_system_async::<CaltrainStatus>(ctx);
    }
}

impl Handler<CaltrainStatus> for Notifier {
    type Result = ();

    fn handle(&mut self, status: CaltrainStatus, _: &mut Self::Context) -> Self::Result {
        if let Some(t) = self.notify_after {
            if Local::now().naive_local().time() < t {
                return;
            }
        }

        let (northbound, southbound) = status.get_status();

        let incoming_trains = if self.direction == Northbound {
            northbound
        } else {
            southbound
        };

        let mut trains_notified = vec![];

        let trains_to_notify = incoming_trains
            .iter()
            .filter(|incoming_train| self.notify_types.contains(&incoming_train.get_train_type()))
            .filter(|incoming_train| incoming_train.get_min_till_departure() <= self.notify_at)
            .filter(|incoming_train| !self.trains_notified.contains(&incoming_train.get_id()));

        for train in trains_to_notify {
            trains_notified.push(train.get_id());
            let notification_result = Notification::new()
                .summary("Caltrain")
                .body(
                    format!(
                        "{} train {} is departing in {} minutes at {}!",
                        train.get_train_type(),
                        train.get_id(),
                        train.get_min_till_departure(),
                        (Local::now() + Duration::minutes(train.get_min_till_departure() as i64))
                            .format("%l:%M%p")
                    )
                    .as_str(),
                )
                .timeout(Timeout::Never)
                .show();
            if let Err(e) = notification_result {
                eprintln!("error creating notification: {}", e);
            }
        }

        self.trains_notified
            .iter()
            .copied()
            .filter(|notified| {
                incoming_trains
                    .iter()
                    .any(|incoming| incoming.get_id() == *notified)
            })
            .for_each(|id| trains_notified.push(id));

        self.trains_notified = trains_notified.into_iter().collect();
    }
}
