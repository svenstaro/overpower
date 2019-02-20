use clap::{_clap_count_exprs, arg_enum};
use futures::future::*;
use futures::prelude::*;
use futures::{Future, Stream};
use reqwest::r#async::{Client, Response};
use reqwest::{Method, Url};
use std::time::{Duration, Instant};
use structopt::clap::AppSettings;
use structopt::StructOpt;

arg_enum! {
    #[derive(Debug)]
    enum HttpMethods {
        GET,
        HEAD,
        POST,
        PUT,
        DELETE,
    }
}

#[derive(StructOpt, Debug)]
#[structopt(
    name = "overpower",
    about = "CLI tool to benchmark web servers with nice output",
    raw(global_settings = "&[AppSettings::ColoredHelp]")
)]
struct Config {
    #[structopt(
        short = "c",
        long = "connections",
        default_value = "2",
        help = "Connections to use"
    )]
    connections: u32,

    #[structopt(
        short = "D",
        long = "duration",
        default_value = "5",
        help = "Duration of benchmark in seconds"
    )]
    duration: u32,

    #[structopt(
        short = "t",
        long = "threads",
        default_value = "2",
        help = "Number of threads to use"
    )]
    threads: u16,

    #[structopt(
        short = "H",
        long = "header",
        help = "Add header to requests, can be passed multiple times"
    )]
    header: Vec<String>,

    #[structopt(
        short = "r",
        long = "rate",
        default_value = "0",
        help = "Requests per second to send, 0 means as fast as possible"
    )]
    rate: u32,

    #[structopt(short = "d", long = "data", help = "Sends the specified data")]
    data: Option<String>,

    #[structopt(
        short = "X",
        long = "request",
        default_value = "GET",
        raw(
            possible_values = "&HttpMethods::variants()",
            case_insensitive = "true"
        ),
        help = "Use a custom request method"
    )]
    method: Method,

    #[structopt(name = "URL")]
    url: Url,
}

/// This struct is required to keep track of when work on its respective TimekeepingFuture was
/// started.
/// We know work has started when it's polled for the first time.
/// When that happens, we note the time when it has started processing in order to later compare it
/// to later times when polling.
#[derive(Debug)]
enum PolledState {
    Unpolled,
    Polled(std::time::Instant),
}

/// A timekeeping `Future` that wraps `F`.
#[derive(Debug)]
struct TimekeepingFuture<F: Future> {
    inner: F,
    state: PolledState,
}

fn timekeeping<F: Future>(future: F) -> TimekeepingFuture<F> {
    TimekeepingFuture {
        inner: future,
        state: PolledState::Unpolled,
    }
}

impl<F: Future> Future for TimekeepingFuture<F> {
    type Item = (Result<F::Item, F::Error>, Duration);
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let t1 = match self.state {
            PolledState::Unpolled => Instant::now(),
            PolledState::Polled(t) => t,
        };

        let duration = Instant::now() - t1;

        match self.inner.poll() {
            Ok(Async::Ready(value)) => Ok(Async::Ready((Ok(value), duration))),
            Ok(Async::NotReady) => {
                self.state = PolledState::Polled(t1);
                Ok(Async::NotReady)
            }
            Err(err) => Err(err),
        }
    }
}

fn fetch() -> impl Future<Item = (), Error = ()> {
    let client = Client::new();

    let output = |mut res: Response| Ok(res.status());

    let mut requests = vec![];

    for i in 1..3 {
        requests.push(client.get("http://0.0.0.0:8080").send().and_then(output))
    }

    let f = join_all(requests);
    f.map(|x| {
        println!("{:?}", x);
    })
    .map_err(|err| {
        println!("stdout error: {}", err);
    })
}

fn main() {
    // let config = Config::from_args();
    tokio::run(fetch());
}
