use anyhow::{Error, Result};
use clap::{arg, Parser};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use serde::{Deserialize, Serialize};
use core::option::Option;
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long="domain, domain name to be bound to the local device ip address")]
    domain: String,

    #[arg(long="disable-proxy, disable Cloudflare proxy")]
    disable_proxy: bool,

    #[arg(short, long="api-key, Cloudflare API Key with Edit Zones Permissions")]
    api_key: String,
}


// fetch all the zones for the domain from cloudflare
#[derive(Deserialize, Debug)]
struct ZoneResponse {
    result: Vec<Zone>,
}

#[derive(Deserialize, Clone, Debug)]
struct Zone {
    id: String,
    name: String,
}

#[derive(Serialize, Clone, Deserialize, Debug)]
struct DNSRecord {
    id: Option<String>,
    name: String,
    content: String,
    #[serde(rename = "type")]
    record_type: String,
    proxied: bool,
    ttl: u32,
}

#[derive(Deserialize, Debug)]
struct DNSRecordResponse {
    result: Vec<DNSRecord>,
    success: bool,
    errors: Vec<String>,
    messages: Vec<String>,
}

async fn fetch_zones_by_domain(domain: String, api_key: String) -> Result<Option<Zone>> {
    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", api_key))?);
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

async fn get_dns_record(zone_id: String, name: String, record_type: String, api_key: String) -> Result<Option<DNSRecord>> {
    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", api_key))?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let response: DNSRecordResponse = client
        .get(format!("https://api.cloudflare.com/client/v4/zones/{}/dns_records", zone_id))
        .query(&[("name", name), ("type", record_type)])
        .headers(headers)
        .send()
        .await?
        .json().await?;

    if response.result.len() == 1 {
        let record = &response.result[0];
        println!("Record: {:?}", record);
        return Ok(Some(record.clone()));
    }
    return Ok(None);
}

async fn update_zone(zone_id: String, name: String, content: String, disable_proxy: &bool, apy_key: String) -> Result<()> {
    let mut url = format!("https://api.cloudflare.com/client/v4/zones/{}/dns_records", zone_id);
    let client = reqwest::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", apy_key))?);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));


    let data: DNSRecord = DNSRecord {
        id: None,
        name: name.to_string(),
        content: content.to_string(),
        record_type: "A".to_string(),
        proxied: !disable_proxy,
        ttl: 1,
    };

    match get_dns_record(zone_id, name, "A".to_string(), apy_key).await {
        Ok(Some(record)) => {
            url = format!("{}/{}", url, record.id.unwrap());
            let response = client
                .put(url)
                .json(&data)
                .headers(headers)
                .send()
                .await?;
            println!("Response status: {}", response.status());
            let body = response.text().await?;
            println!("Response body: {}", body);
        }
        Ok(None) => {
            let response = client
                .post(url)
                .json(&data)
                .headers(headers)
                .send()
                .await?;
            let body = response.text().await?;
            println!("Response status: {}", body);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
    Ok(())
}

fn parse_root_domain(domain: String) -> Result<String, Error> {
    let lower_case_domain = domain.to_lowercase();
    let url_str = if lower_case_domain.starts_with("http") || lower_case_domain.starts_with("https") {
        domain.to_string()
    } else {
        format!("https://{}", domain)
    };
    let host = url::Url::parse(&url_str).unwrap().host_str().ok_or("Invalid domain").unwrap().to_string();

    let parts = host.split('.').collect::<Vec<&str>>();
    if parts.len() <= 2 {
        Ok(host)
    } else {
        Ok(parts[parts.len() - 2..].join("."))
    }
}

fn create_updater(zone: Option<Zone>, domain: Arc<String>, disable_proxy: Arc<bool>, api_key: Arc<String>) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        if let Some(zone) = zone {
            let zid = zone.id.clone();
            loop {
                let current_ip = get_current_ip().await.unwrap();
                println!("{}", current_ip);
                update_zone(zid.clone(), domain.as_str().to_string(), current_ip, disable_proxy.as_ref(), api_key.as_str().to_string()).await.unwrap();
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        }
        Ok(())
    })
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let domain = Arc::new(args.domain);
    let api_key = Arc::new(args.api_key);
    let disable_proxy = Arc::new(args.disable_proxy);
    let root_domain = parse_root_domain(domain.to_string());
    let zones = fetch_zones_by_domain(root_domain?, api_key.to_string()).await?;

    let current_zone = match zones {
        Some(zone) => {
            println!("Zone: {:?}", zone);
            Some(zone)
        }
        None => {
            println!("No zone found for domain: {}", domain.to_string());
            None
        }
    };

    let updater: JoinHandle<Result<()>> = create_updater(current_zone, domain, disable_proxy, api_key);
    tokio::try_join!(updater)?;
    Ok(())
}
