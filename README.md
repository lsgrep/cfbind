# cfbind
> dynamic DNS with Cloudflare



### Install
- `cargo build`

### How to use

```
./cfbind --help
Usage: cfbind [OPTIONS] --domain, domain name to be bound to the local device ip address <DOMAIN> --api_key, Cloudflare API Key with Edit Zones Permissions <API_KEY>

Options:
  -d, --domain, domain name to be bound to the local device ip address <DOMAIN>
      --disable-proxy, disable Cloudflare proxy
  -a, --api_key, Cloudflare API Key with Edit Zones Permissions <API_KEY>
  -h, --help                                                                     Print help
  -V, --version                                                                  Print version
```