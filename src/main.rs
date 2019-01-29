use clap::{_clap_count_exprs, arg_enum};
use futures::{Future, Stream};
use reqwest::r#async::{Client, Decoder};
use reqwest::{Method, Url};
use std::io::{self, Cursor};
use std::mem;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use futures::prelude::*;

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

// fn make_reques()

fn run_benchmark(config: Config) -> impl Future<Item = (), Error = ()> {
    Client::new()
        .request(config.method, config.url)
        .send()
        .and_then(|mut res| {
            println!("{}", res.status());

            let body = mem::replace(res.body_mut(), Decoder::empty());
            body.concat2()
        })
        .map_err(|err| println!("request error: {}", err))
        .map(|body| {
            let mut body = Cursor::new(body);
            let _ = io::copy(&mut body, &mut io::stdout()).map_err(|err| {
                println!("stdout error: {}", err);
            });
        })
}

fn main() {
    let config = Config::from_args();
    tokio::run(run_benchmark(config));
}
