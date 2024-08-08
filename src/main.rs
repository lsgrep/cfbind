use anyhow::{anyhow, Context};
use anyhow::{Error, Result};
use clap::{arg, Parser};
use cloudflare::endpoints::dns::{DnsContent, DnsRecord, Meta};
use cloudflare::endpoints::zone::Zone;
use cloudflare::framework::auth::Credentials;
use cloudflare::framework::response::{ApiResponse, ApiSuccess};
use cloudflare::framework::{async_api, Environment, HttpApiClientConfig};
use core::option::Option;
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long = "domain, domain name to be bound to the local device ip address"
    )]
    domain: String,

    #[arg(long = "disable-proxy, disable Cloudflare proxy")]
    disable_proxy: bool,

    #[arg(
        short,
        long = "api-key, Cloudflare API Key with Edit Zones Permissions, can also be set as an environment variable CF_API_KEY"
    )]
    api_key: Option<String>,
}

pub async fn get_zones(api_client: &async_api::Client) -> anyhow::Result<HashMap<String, Zone>> {
    let result: ApiResponse<Vec<Zone>> = api_client
        .request(&cloudflare::endpoints::zone::ListZones {
            params: Default::default(),
        })
        .await;
    match result {
        Ok(apiResp) => {
            let zones = apiResp.result;
            let mut zone_map = HashMap::new();
            for zone in zones {
                zone_map.insert(zone.name.clone(), zone);
            }
            Ok(zone_map)
        }
        Err(e) => {
            log::error!("Error: {:#?}", e);
            Err(anyhow::anyhow!("Error: {:#?}", e))
        }
    }
}

pub fn root_domain_name(name: String) -> String {
    let nc = name.clone();
    let parts = nc.split('.').collect::<Vec<&str>>();
    if parts.len() <= 2 {
        nc
    } else {
        parts[parts.len() - 2..].join(".")
    }
}

pub async fn get_zone(api_client: &async_api::Client, name: &str) -> anyhow::Result<Zone> {
    let mut zones = get_zones(api_client).await?;
    let root_domain = root_domain_name(name.to_string());
    zones.remove(&root_domain).context("Zone not found")
}

pub async fn get_dns_record(
    api_client: &async_api::Client,
    name: &str,
) -> anyhow::Result<Option<DnsRecord>> {
    let zone = get_zone(api_client, name).await?;
    let mut response: ApiSuccess<Vec<DnsRecord>> = api_client
        .request(&cloudflare::endpoints::dns::ListDnsRecords {
            zone_identifier: zone.id.as_str(),
            params: cloudflare::endpoints::dns::ListDnsRecordsParams {
                name: Some(name.to_string()),
                ..Default::default()
            },
        })
        .await?;
    Ok(response.result.into_iter().next())
}

pub async fn update_dns_record(
    api_client: &async_api::Client,
    name: &str,
    dns_content: DnsContent,
    proxied: bool,
) -> anyhow::Result<()> {
    let dns_record: Option<DnsRecord> = get_dns_record(api_client, name).await?;
    let zone = get_zone(api_client, name).await?;
    log::info!("DNS Record: {:#?}", dns_record);
    let result = match dns_record {
        Some(record) => {
            api_client
                .request(&cloudflare::endpoints::dns::UpdateDnsRecord {
                    zone_identifier: record.zone_id.as_str(),
                    identifier: record.id.as_str(),
                    params: cloudflare::endpoints::dns::UpdateDnsRecordParams {
                        ttl: Some(1),
                        proxied: Some(proxied),
                        name,
                        content: dns_content,
                    },
                })
                .await
        }
        None => {
            api_client
                .request(&cloudflare::endpoints::dns::CreateDnsRecord {
                    zone_identifier: zone.id.as_str(),
                    params: cloudflare::endpoints::dns::CreateDnsRecordParams {
                        name,
                        content: dns_content,
                        proxied: Some(proxied),
                        ttl: Some(1),
                        priority: None,
                    },
                })
                .await
        }
    };
    match result {
        Ok(apiResp) => {
            log::info!("DNS Record Updated: {:#?}", apiResp.result);
            Ok(())
        }
        Err(e) => {
            log::error!("Error: {:#?}", e);
            Err(anyhow::anyhow!("Error: {:#?}", e))
        }
    }
}

async fn get_current_ip() -> Result<String> {
    let response = reqwest::get("https://api.ipify.org").await?.text().await?;
    Ok(response)
}

fn create_updater(
    api_key: Arc<String>,
    domain: Arc<String>,
    disable_proxy: Arc<bool>,
) -> JoinHandle<Result<()>> {
    let creds = Credentials::UserAuthToken {
        token: api_key.to_string(),
    };
    let cf_api_client = async_api::Client::new(
        creds,
        HttpApiClientConfig::default(),
        Environment::Production,
    );

    match cf_api_client {
        Ok(client) => {
            tokio::spawn(async move {
                loop {
                    let current_ip = get_current_ip().await.unwrap();
                    log::info!("{}", current_ip);
                    // parse string as ip
                    let record = DnsContent::A {
                        content: Ipv4Addr::from_str(current_ip.as_str())?,
                    };
                    update_dns_record(
                        &client,
                        domain.as_str(),
                        record,
                        disable_proxy.as_ref().clone(),
                    )
                    .await
                    .unwrap();
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                }
            })
        }
        Err(e) => tokio::spawn(async move { Err(anyhow!(e)) }),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let domain = Arc::new(args.domain);
    let api_key = Arc::new(
        args.api_key
            .unwrap_or_else(|| std::env::var("CF_API_KEY").unwrap()),
    );
    let disable_proxy = Arc::new(args.disable_proxy);
    let updater: JoinHandle<Result<()>> = create_updater(api_key, domain, disable_proxy);
    tokio::try_join!(updater)?;
    Ok(())
}
