use anyhow::Result;
use clap::{arg, Parser};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rootDomain: String,

    #[arg(short, long)]
    domain: String,

    #[arg(short, long)]
    apiKey: String,
}


// fetch all the zones for the domain from cloudflare
#[derive(Deserialize, Debug)]
struct ZoneResponse {
    result: Vec<Zone>,
}

#[derive(Deserialize, Debug)]
struct Zone {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct DnsRecord {
    name: String,
    content: String,
    #[serde(rename = "type")]
    record_type: String,
    proxied: bool,
    ttl: u32,
}

async fn fetch_zones_by_domain(domain: &str, apiKey: &str) -> Result<Option<Zone>> {
    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", apiKey))?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let url = format!("https://api.cloudflare.com/client/v4/zones?name={}", domain);
    let response: ZoneResponse = client
        .get(url)
        .headers(headers)
        .send()
        .await?
        .json()
        .await?;
    Ok(response.result.into_iter().next())
}

async fn get_current_ip() -> Result<String> {
    let response = reqwest::get("https://api.ipify.org")
        .await?
        .text()
        .await?;
    Ok(response)
}

async fn update_zone(zone_id: &str, name: &str, content: &str, apiKey: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", apiKey))?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let url = format!("https://api.cloudflare.com/client/v4/zones/{}/dns_records", zone_id);

    let data: DnsRecord = DnsRecord {
        name: name.to_string(),
        content: content.to_string(),
        record_type: "A".to_string(),
        proxied: true,
        ttl: 1,
    };
    let response = client
        .post(url)
        .json(&data)
        .headers(headers)
        .send()
        .await?;

    println!("Response status: {}", response.status());

    let body = response.text().await?;
    println!("Response body: {}", body);
    Ok(())
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let zones = fetch_zones_by_domain(&args.rootDomain, &args.apiKey).await?;
    if let Some(zone) = zones {
        println!("Zone: {:?}", zone);

        let current_ip = get_current_ip().await?;
        println!("{}", current_ip);

        update_zone(&zone.id, &args.domain, &current_ip, &args.apiKey).await?;
    } else {
        println!("No zone found for domain: {}", args.domain);
    }
    Ok(())
}
