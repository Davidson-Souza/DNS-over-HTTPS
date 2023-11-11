# DNS Over HTTPS proxy

This is a super basic, portable and efficient proxy that listens for UDP connections and proxies 
the traffic over https (and optionally over a proxy). The goal here is twofold:

 - Privacy: DNS queries aren't encrypted, therefore, any passive attacker spoffing your connection can learn
            about which domains you access. This is a major privacy leak that is sometimes used by ISPs to
            sell this data to data brokers. Using encryption (and Tor) removes this leak entirely.

 - Security: The DNS protocol does't define any means of authentication, opening some huge security 
             holes, like DNS hijack. TLS, on the other hand, uses has strong authentication mechanisms
             based on a PKI system. It's been battle-tested for years now, and works fine. By using 
             HTTPS instead of raw DNS, we make sure that we only have to trust the DNS server, not
             random actors that stands between you and the server.

## How it works?

DNS over HTTPS is define on RFC-8686, and uses a HTTPS POST to send the DNS request. You simply send a 
binary body, with content type `application/dns-message`. The DNS request package is wrapped inside the
body.

This daemon listens for connections over an UDP port (usually 53) and every request sent to it becomes 
an HTTPS request. If set, the request will be sent over a proxy, for maximum privacy, we recommend using
a Tor SOCKS proxy.

To decrease latency, all requests can be cached. We use a simple in-memory cache that allows for almost
instant retrieval. You can disable the cache by ommiting the `-c` flag at startup.

### Usage

```bash
dns-SoH [-c] [-l] --remote <DoH_server> --addr-bind <addr_to_bind> 
```

Example:

```bash
dns-SoH -lc --remote https://1.1.1.1/dns-query --addr-bind 127.0.0.1:53
```

## List of DoH-ready servers
```
+-----------+---------------------------+---------+
|   Name    |          URL              | Comment |
+-----------+---------------------------+---------+
| Clodflare | https://1.1.1.1/dns-query |         |
+-----------+---------------------------+---------+
```
