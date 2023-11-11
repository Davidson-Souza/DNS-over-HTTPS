use clap::Parser;
use reqwest::Client;
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::Instant,
};

/// RFC-8686 mandates that the content-type header should be set to application/dns-message
const POST_CONTENT_TYPE_KEY: &str = "Content-Type";
const POST_CONTENT_TYPE_VALUE: &str = "application/dns-message";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let listener = UdpSocket::bind(args.addr_bind).unwrap();
    let mut cache: HashMap<String, (Instant, Vec<u8>)> = HashMap::new();

    // Allocate a 65kb buffer, this should be more than enougth for most applications
    let mut buffer = [0; 1 << 16];

    // We only handle one client per time
    while let Ok((count, origin)) = listener.recv_from(&mut buffer) {
        let client = reqwest::Client::new();
        let request = buffer[0..count].to_vec();
        let mut name = String::new();
        get_query_names(&request[12..], &mut name);
        cache_hit(&client, &mut cache, &args.remote, &name, request.clone()).await?;

        let (_, res) = cache.get(&name).unwrap();
        let res = request[0..12].iter().chain(res.into_iter()).copied().collect::<Vec<_>>();
        listener.send_to(res.as_slice(), origin)?;
    }

    Ok(())
}
async fn cache_miss<'a>(
    client: &Client,
    cache: &mut HashMap<String, (Instant, Vec<u8>)>,
    remote: &str,
    name: &String,
    request: Vec<u8>,
) -> anyhow::Result<()> {
    let post = client
        .post(remote)
        .body(request)
        .header(POST_CONTENT_TYPE_KEY, POST_CONTENT_TYPE_VALUE)
        .send()
        .await?;
    let mut body = post.bytes().await?.to_vec();
    cache.insert(name.to_string(), (Instant::now(), body.drain(12..).collect()));
    Ok(())
}
async fn cache_hit(
    client: &Client,
    cache: &mut HashMap<String, (Instant, Vec<u8>)>,
    remote: &str,
    name: &String,
    request: Vec<u8>,
) -> anyhow::Result<()> {
    if let Some((ttl, cached)) = cache.get(name).cloned() {
        if !ttl.elapsed().as_secs() > 3600 {
            println!("{name} HIT");
            let mut res = request[0..12].to_vec();
            res.extend(cached.iter());
            return Ok(());
        }
    }

    println!("{name} MISS");
    cache_miss(client, cache, remote, name, request).await?;
    Ok(())
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
}
