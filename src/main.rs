// SPDX Licence identifier: MIT
// Copyright (C): 2023 Davidson Souza <davidson.lucas.souza@outlook.com>

//! A super simple implementation of RFC8686 DNS over HTTPS that proxies normal UDP/53
//! requests into a DoH-enabled server, possibly over a SOCKS proxy. This client isn't
//! meant to be exposed over the internet, it can only handle one client at the time and
//! can memory leak very easily.
//!
//! If you need a small-footprint service that proxies DNS queries over HTTPS and nothing
//! else, this may be a good fit for you.

use clap::Parser;
use log::debug;
use reqwest::{Client, Proxy};
use simplelog::{Config, SharedLogger};
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::Instant,
};
type Cache = HashMap<Vec<u8>, (Instant, Vec<u8>)>;

/// RFC-8686 mandates that the content-type header should be set to application/dns-message
const POST_CONTENT_TYPE_KEY: &str = "Content-Type";
const POST_CONTENT_TYPE_VALUE: &str = "application/dns-message";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    // Gets the cache ttl param
    let ttl = if args.cache {
        args.cache_ttl
    } else {
        0 // this just disables caching (every entry is stale, no metter what)
    };

    // Init global logger
    if args.log_queries {
        init_logger(None, log::LevelFilter::Debug, true);
    }

    // The UDP socket we'll listen for incomming DNS requests
    let listener = UdpSocket::bind(args.addr_bind).unwrap();

    // A cache used for domains speeding up dns requests
    let mut cache: Cache = HashMap::new();

    // Allocate a 65kb buffer, this should be more than enougth for most applications
    let mut buffer = [0; 1 << 16];

    // A https client we use to make DoH requests
    let client = if let Some(proxy) = args.proxy {
        reqwest::Client::builder()
            .proxy(Proxy::all(proxy)?)
            .build()?
    } else {
        reqwest::Client::builder().build()?
    };

    // We only handle one client at the time
    while let Ok((count, origin)) = listener.recv_from(&mut buffer) {
        let request = buffer[0..count].to_vec();

        // Retrieve the query paramenter to log
        let mut name = String::new();
        get_query_names(&request[12..], &mut name);

        // Remove expired entries from our cache
        invalidate_cache(&mut cache, ttl, false);
        cache_hit(&client, &mut cache, &args.remote, &name, request.clone()).await?;

        let (_, res) = cache.get(&request[2..]).unwrap();
        let res = request[0..2]
            .iter()
            .chain(res.into_iter())
            .copied()
            .collect::<Vec<_>>();
        listener.send_to(res.as_slice(), origin)?;
    }

    Ok(())
}

/// Transverses a cache and retains only entryies that are not stale yet
fn invalidate_cache(cache: &mut Cache, ttl: u64, force: bool) {
    let mut new_cache = Cache::new();
    for key in cache.keys().cloned() {
        let value = cache.get(&key).unwrap();
        if value.0.elapsed().as_secs() > ttl || !force {
            new_cache.insert(key.clone(), value.clone());
        }
    }
    *cache = new_cache;
}

/// If we don't have a particular element cached, get it and insert in our local cache
async fn cache_miss<'a>(
    client: &Client,
    cache: &mut Cache,
    remote: &str,
    request: Vec<u8>,
) -> anyhow::Result<()> {
    let post = client
        .post(remote)
        .body(request.clone())
        .header(POST_CONTENT_TYPE_KEY, POST_CONTENT_TYPE_VALUE)
        .send()
        .await?;
    let mut body = post.bytes().await?.to_vec();
    cache.insert(
        request[2..].to_vec(),
        (Instant::now(), body.drain(2..).collect()),
    );
    Ok(())
}

/// Checks if we have this entry on cache, if we do, this function is a no-op.
/// If we don't have this entry cached, we request it.
///
/// After calling this function, you're garanteed to have that entry cached, so
/// cache.get(Key).unwrap() will never panic
async fn cache_hit(
    client: &Client,
    cache: &mut Cache,
    remote: &str,
    name: &String,
    request: Vec<u8>,
) -> anyhow::Result<()> {
    if cache.contains_key(&request[2..].to_vec()) {
        debug!("{} HIT", name);
        return Ok(());
    }
    debug!("{} MISS", name);
    cache_miss(client, cache, remote, request).await
}
fn get_query_names(req: &[u8], acc: &mut String) {
    let len = req[0] as usize;
    if len == 0 {
        return;
    }
    let head = &req[1..=len];
    let tail = &req[(len + 1)..];
    acc.extend(
        head.iter()
            .map(|c| unsafe { char::from_u32_unchecked(*c as u32) }),
    );
    acc.push('.');
    get_query_names(tail, acc);
}

fn init_logger(log_file: Option<&str>, log_level: log::LevelFilter, log_to_term: bool) {
    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![];
    if let Some(file) = log_file {
        let file_logger = simplelog::WriteLogger::new(
            log_level,
            Config::default(),
            std::fs::File::create(file).unwrap(),
        );
        loggers.push(file_logger);
    }
    if log_to_term {
        let term_logger = simplelog::TermLogger::new(
            log_level,
            Config::default(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        );
        loggers.push(term_logger);
    }
    if loggers.is_empty() {
        eprintln!("No logger specified, logging disabled");
        return;
    }
    let _ = simplelog::CombinedLogger::init(loggers);
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The DoH-enabled server that will cache your requests
    #[arg(short, long, value_name = "URL")]
    remote: String,
    /// A local addr to bind to (e.g. 127.0.0.1:53)
    #[arg(short, long, value_name = "ADDR", default_value = "127.0.0.1:53")]
    addr_bind: SocketAddr,
    /// Whether to cache requests
    #[arg(short, long, default_value_t = false)]
    cache: bool,
    #[arg(short, long, default_value_t = false)]
    log_queries: bool,
    #[arg(short = 't', long, default_value_t = 3600)]
    cache_ttl: u64,
    #[arg(short = 'p', long, default_value = None)]
    proxy: Option<String>,
}
