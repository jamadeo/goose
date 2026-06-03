pub use goose_providers::api_client::*;

pub fn tls_config_from_goose_config() -> anyhow::Result<Option<TlsConfig>> {
    let config = crate::config::Config::global();
    let mut tls_config = TlsConfig::new();
    let mut has_tls_config = false;

    let client_cert_path = config.get_param::<String>("GOOSE_CLIENT_CERT_PATH").ok();
    let client_key_path = config.get_param::<String>("GOOSE_CLIENT_KEY_PATH").ok();

    match (client_cert_path, client_key_path) {
        (Some(cert_path), Some(key_path)) => {
            tls_config = tls_config.with_client_cert_and_key(cert_path.into(), key_path.into());
            has_tls_config = true;
        }
        (Some(_), None) => {
            anyhow::bail!(
                "Client certificate provided (GOOSE_CLIENT_CERT_PATH) but no private key (GOOSE_CLIENT_KEY_PATH)"
            );
        }
        (None, Some(_)) => {
            anyhow::bail!(
                "Client private key provided (GOOSE_CLIENT_KEY_PATH) but no certificate (GOOSE_CLIENT_CERT_PATH)"
            );
        }
        (None, None) => {}
    }

    if let Ok(ca_cert_path) = config.get_param::<String>("GOOSE_CA_CERT_PATH") {
        tls_config = tls_config.with_ca_cert(ca_cert_path.into());
        has_tls_config = true;
    }

    Ok(has_tls_config.then_some(tls_config))
}

pub fn api_client_from_goose_config(host: String, auth: AuthMethod) -> anyhow::Result<ApiClient> {
    api_client_from_goose_config_with_timeout(
        host,
        auth,
        std::time::Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECS),
    )
}

pub fn api_client_from_goose_config_with_timeout(
    host: String,
    auth: AuthMethod,
    timeout: std::time::Duration,
) -> anyhow::Result<ApiClient> {
    ApiClient::with_timeout_and_tls_config(host, auth, timeout, tls_config_from_goose_config()?)
}
