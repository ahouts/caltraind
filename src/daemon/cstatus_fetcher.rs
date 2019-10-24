use std::time::Duration;

use actix::prelude::*;
use actix_broker::{BrokerIssue, SystemBroker};
use actix_web::client::Client;
use futures::{compat::Future01CompatExt, FutureExt, TryFutureExt};

use crate::caltrain_status::CaltrainStatus;
use crate::station::Station;

pub struct CStatusFetcher {
    station: Station,
    duration: Duration,
}

impl CStatusFetcher {
    pub fn new(station: Station, duration: Duration) -> Self {
        CStatusFetcher { station, duration }
    }

    async fn update_status(station: Station) -> Result<CaltrainStatus, String> {
        let result = Client::default()
            .get(station.get_url())
            .send()
            .compat()
            .await;
        let mut resp = match result {
            Ok(resp) => resp,
            Err(e) => {
                return Err(format!("error making request to caltrain: {}", e));
            }
        };
        let bytes = match resp.body().compat().await {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(format!("invalid payload from caltrain: {}", e));
            }
        };
        let text = match String::from_utf8(bytes.to_vec()) {
            Ok(text) => text,
            Err(e) => {
                return Err(format!(
                    "error while parsing resposne from caltrain as utf-8: {}",
                    e
                ));
            }
        };
        match CaltrainStatus::from_html(text) {
            Ok(cstatus) => Ok(cstatus),
            Err(e) => Err(format!("error parsing caltrain xml: {}", e)),
        }
    }

    fn run_status_update(&mut self, ctx: &mut <CStatusFetcher as Actor>::Context) {
        let status_update_future = CStatusFetcher::update_status(self.station)
            .unit_error()
            .boxed_local()
            .compat();
        let wrapped = actix::fut::wrap_future::<_, Self>(status_update_future);
        let emitted = wrapped.map(|result, actor, _| match result {
            Ok(cstatus) => actor.issue_async::<SystemBroker, _>(cstatus),
            Err(msg) => eprintln!("{}", msg),
        });
        ctx.spawn(emitted);
    }
}

impl Actor for CStatusFetcher {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(self.duration, |csf: &mut Self, ctx: &mut Self::Context| {
            csf.run_status_update(ctx)
        });
        self.run_status_update(ctx);
    }
}
