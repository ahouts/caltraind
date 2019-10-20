use std::collections::BTreeSet;

use actix::prelude::*;
use actix_broker::BrokerSubscribe;
use notify_rust::{Notification, Timeout};

use crate::caltrain_status::Direction::Northbound;
use crate::caltrain_status::{CaltrainStatus, Direction, TrainType};

pub struct Notifier {
    trains_notified: BTreeSet<u16>,
    notify_at: u16,
    notify_types: BTreeSet<TrainType>,
    direction: Direction,
}

impl Notifier {
    pub fn new(notify_types: BTreeSet<TrainType>, notify_at: u16, direction: Direction) -> Self {
        Notifier {
            notify_at,
            notify_types,
            trains_notified: BTreeSet::new(),
            direction,
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
                        "{} train {} is leaving in {} minutes!",
                        train.get_train_type(),
                        train.get_id(),
                        train.get_min_till_departure()
                    )
                    .as_str(),
                )
                .timeout(Timeout::Milliseconds(10_000))
                .show();
            if let Err(e) = notification_result {
                eprintln!("error creating notification: {}", e);
            }
        }

        self.trains_notified
            .iter()
            .map(|id| *id)
            .filter(|notified| {
                incoming_trains
                    .iter()
                    .any(|incoming| incoming.get_id() == *notified)
            })
            .for_each(|id| trains_notified.push(id));

        self.trains_notified = trains_notified.into_iter().collect();
    }
}
