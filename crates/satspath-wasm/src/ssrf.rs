//! SSRF protection for WASM resolvers.

/// Validate that a URL is safe to request (no SSRF risk).
/// In a WASM context, we can't easily resolve DNS to check IPs before fetching,
/// but we CAN block known bad hostnames and IP literals.
pub fn validate_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;

    // Must be HTTPS
    if parsed.scheme() != "https" {
        return Err(format!("Blocked scheme '{}' — only HTTPS is allowed", parsed.scheme()));
    }

    let host = parsed.host_str().ok_or_else(|| "URL has no host".to_string())?;
    let host_lower = host.to_ascii_lowercase();

    let blocked_hosts = [
        "localhost",
        "localhost.localdomain",
        "ip6-localhost",
        "ip6-loopback",
        "metadata.google.internal",
        "169.254.169.254",
    ];

    if blocked_hosts.iter().any(|blocked| host_lower == *blocked || host_lower.ends_with(&format!(".{blocked}"))) {
        return Err(format!("Blocked host: {host} (internal/metadata endpoint)"));
    }

    // Block IPv4 loopback, private, link-local, carrier-grade NAT
    if let Some(first_octet) = extract_first_octet(&host_lower) {
        if first_octet == 127 || first_octet == 10 || first_octet == 172 || first_octet == 192 || first_octet == 169 || first_octet == 100 {
            return Err(format!("Blocked IP: {host} (private/reserved range)"));
        }
    }

    // Block IPv6 loopback
    if host_lower == "[::1]" || host_lower.starts_with("[fe80:") || host_lower.starts_with("[fc00:") || host_lower.starts_with("[fd00:") {
        return Err(format!("Blocked IPv6: {host} (private/reserved range)"));
    }

    Ok(())
}

fn extract_first_octet(host: &str) -> Option<u8> {
    if host.chars().all(|c| c.is_ascii_digit() || c == '.') {
        if let Some(first) = host.split('.').next() {
            return first.parse::<u8>().ok();
        }
    }
    None
}
