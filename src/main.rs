use clap::arg_enum;
use futures::future::*;
use futures::prelude::*;
use futures::Future;
use lazy_static::lazy_static;
use reqwest::r#async::Client;
use reqwest::{Method, Url};
use std::time::{Duration, Instant};
use structopt::clap::AppSettings;
use structopt::StructOpt;
use tokio::runtime::Builder;

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

pub fn duration_from_str_secs(d: &str) -> Result<Duration, std::num::ParseIntError> {
    let secs: u64 = d.parse()?;
    Ok(Duration::from_secs(secs))
}

lazy_static! {
    static ref NUM_CPUS: String = num_cpus::get().to_string();
}

#[derive(StructOpt, Debug)]
#[structopt(
    name = "overpower",
    author,
    about,
    global_settings = &[AppSettings::ColoredHelp],
)]
struct Config {
    // #[structopt(
    //     short = "c",
    //     long = "concurrency",
    //     default_value = "10",
    //     help = "Maximum number of concurrent requests"
    // )]
    // connections: u32,
    /// Duration of the benchmark in seconds
    #[structopt(
        short = "D",
        long = "duration",
        default_value = "5",
        parse(try_from_str = duration_from_str_secs),
    )]
    duration: Duration,

    /// Number of threads to use
    #[structopt(
        short = "t",
        long = "threads",
        default_value = &NUM_CPUS,
    )]
    threads: usize,

    /// Add header to requests, can be passed multiple times
    #[structopt(short = "H", long = "header")]
    header: Vec<String>,

    /// Number of new requests to spawn per second, 0 means as fast as possible
    #[structopt(short = "r", long = "rate", default_value = "0")]
    rate: u32,

    #[structopt(short = "d", long = "data", help = "Sends the specified data")]
    data: Option<String>,

    /// Use a custom request method
    #[structopt(
        short = "X",
        long = "request",
        default_value = "GET",
        possible_values = &HttpMethods::variants(),
        case_insensitive = true,
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

// fn keep_spawning_requests() -> impl Future<Item = (), Error = ()> {}

fn async_main(config: Config) -> impl Future<Item = (), Error = ()> {
    let client = Client::new();

    // let output = |res: Response| Ok(res.status());

    // let mut requests = vec![];

    let start_time = Instant::now();

    // Keep track of all request start times so that can achieve constant thoughput request
    // spawning.
    // This is used as a sliding window since the spawn rate spreads the given number over a
    // second.
    // It's a queue: The oldest requests are near the start of the Vec while the recent ones are
    // near the end.
    let mut request_start_times = vec![];

    let loop_till_done = loop_fn(vec![], move |mut requests| {
        let now = Instant::now();
        let done = now - start_time >= config.duration;

        if done {
            Ok(Loop::Break(requests))
        } else {
            let num_requests_started_within_last_second = request_start_times
                .iter()
                .filter(|&&x| x < now - Duration::from_secs(1))
                .count();

            let fut = timekeeping(client.get(config.url.clone()).send());

            if config.rate > 0 {
                if let Some(last_request_start_time) = request_start_times.last() {
                    let time_between_requests = Duration::from_secs_f64(1.0 / config.rate as f64);
                    if now - *last_request_start_time > time_between_requests {
                        let lol = tokio::spawn(fut.map(|_| ()).map_err(|_| ()));
                        request_start_times.push(now);
                        requests.push(lol);
                    }
                } else {
                    let lol = tokio::spawn(fut.map(|_| ()).map_err(|_| ()));
                    request_start_times.push(now);
                    requests.push(lol);
                }
            } else {
                let lol = tokio::spawn(fut.map(|_| ()).map_err(|_| ()));
                request_start_times.push(now);
                requests.push(lol);
            }

            Ok(Loop::Continue(requests))
        }
    });

    // loop_till_done.map(|_| ())

    loop_till_done.map(|x| {
        println!("{:?}", x);
        // for thing in x {
        // println!("{:?}", thing.state);
        // }
    })

    // .map_err(|err| {
    //     println!("stdout error: {}", err);
    // })

    // for _ in 0..10 {
    //     requests.push(timekeeping(
    //         client.get(config.url.clone()).send().and_then(output),
    //     ))
    // }

    // let f = join_all(requests);
    // f.map(|x| {
    //     println!("{:?}", x);
    // })
    // .map_err(|err| {
    //     println!("stdout error: {}", err);
    // })
    // let f = select_all(requests);
    // f.map(|x| {
    //     println!("{:?}", x.0);
    // })
    // .map_err(|err| {
    //     // println!("stdout error: {}", err);
    // })
}

// fn fetch() -> impl Future<Item = (), Error = ()> {
// }

fn main() {
    let config = Config::from_args();

    let mut entered = tokio_executor::enter().expect("nested tokio::run");
    let mut runtime = Builder::new()
        .core_threads(config.threads)
        .build()
        .expect("Couldn't create tokio runtime");
    runtime.spawn(lazy(|| async_main(config)));
    entered
        .block_on(runtime.shutdown_on_idle())
        .expect("shutdown cannot error")
}
