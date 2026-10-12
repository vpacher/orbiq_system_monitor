//! TLS setup for the MQTT connection. Builds the rustls client configuration
//! from the config file instead of rumqttc's defaults, which panic when the
//! system certificate store cannot be loaded or contains an unparseable
//! certificate.

use crate::config::MqttConfig;
use rumqttc::tokio_rustls::rustls::pki_types::pem::PemObject;
use rumqttc::tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rumqttc::tokio_rustls::rustls::{ClientConfig, RootCertStore};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum TlsError {
    /// A PEM file could not be read or contained no usable item.
    Pem(PathBuf, String),
    /// Only one of `client_cert_file` / `client_key_file` is set.
    IncompleteClientAuth,
    /// No CA file configured and the system store yielded no certificate.
    NoSystemRoots(String),
    /// rustls rejected the certificate/key combination.
    Rustls(String),
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsError::Pem(path, e) => write!(f, "{}: {e}", path.display()),
            TlsError::IncompleteClientAuth => {
                write!(f, "client_cert_file and client_key_file must be set together")
            }
            TlsError::NoSystemRoots(e) => write!(
                f,
                "no CA certificates in the system store ({e}); set mqtt.ca_file"
            ),
            TlsError::Rustls(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TlsError {}

/// The rustls client configuration for `config`, or `None` when TLS is off.
pub fn client_config(config: &MqttConfig) -> Result<Option<ClientConfig>, TlsError> {
    if !config.tls {
        return Ok(None);
    }
    let roots = match &config.ca_file {
        Some(path) => roots_from_file(path)?,
        None => system_roots()?,
    };
    let builder = ClientConfig::builder().with_root_certificates(roots);
    let tls = match (&config.client_cert_file, &config.client_key_file) {
        (None, None) => builder.with_no_client_auth(),
        (Some(cert), Some(key)) => builder
            .with_client_auth_cert(certificates(cert)?, private_key(key)?)
            .map_err(|e| TlsError::Rustls(e.to_string()))?,
        _ => return Err(TlsError::IncompleteClientAuth),
    };
    Ok(Some(tls))
}

fn certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let certs = CertificateDer::pem_file_iter(path)
        .map_err(|e| TlsError::Pem(path.to_path_buf(), e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Pem(path.to_path_buf(), e.to_string()))?;
    if certs.is_empty() {
        return Err(TlsError::Pem(path.to_path_buf(), "no certificate found".to_string()));
    }
    Ok(certs)
}

fn private_key(path: &Path) -> Result<PrivateKeyDer<'static>, TlsError> {
    PrivateKeyDer::from_pem_file(path).map_err(|e| TlsError::Pem(path.to_path_buf(), e.to_string()))
}

fn roots_from_file(path: &Path) -> Result<RootCertStore, TlsError> {
    let mut roots = RootCertStore::empty();
    for cert in certificates(path)? {
        roots
            .add(cert)
            .map_err(|e| TlsError::Pem(path.to_path_buf(), e.to_string()))?;
    }
    Ok(roots)
}

fn system_roots() -> Result<RootCertStore, TlsError> {
    let loaded = rustls_native_certs::load_native_certs();
    for e in &loaded.errors {
        eprintln!("TLS: skipping part of the system certificate store: {e}");
    }
    let mut roots = RootCertStore::empty();
    // Unparseable certificates are skipped, not fatal: one bad entry in the
    // system store must not take the whole connection down.
    let (_added, skipped) = roots.add_parsable_certificates(loaded.certs);
    if skipped > 0 {
        eprintln!("TLS: skipped {skipped} unparseable system CA certificate(s)");
    }
    if roots.is_empty() {
        let reason = loaded
            .errors
            .first()
            .map_or_else(|| "store is empty".to_string(), ToString::to_string);
        return Err(TlsError::NoSystemRoots(reason));
    }
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tls").join(name)
    }

    fn tls_config() -> MqttConfig {
        MqttConfig {
            tls: true,
            ca_file: Some(fixture("ca.crt")),
            ..MqttConfig::default()
        }
    }

    #[test]
    fn tls_off_gives_no_config() {
        assert!(client_config(&MqttConfig::default()).unwrap().is_none());
    }

    #[test]
    fn ca_file_is_loaded() {
        assert!(client_config(&tls_config()).unwrap().is_some());
    }

    #[test]
    fn client_certificate_and_key_are_accepted() {
        let config = MqttConfig {
            client_cert_file: Some(fixture("client.crt")),
            client_key_file: Some(fixture("client.key")),
            ..tls_config()
        };
        let tls = client_config(&config).unwrap().unwrap();
        assert!(tls.client_auth_cert_resolver.has_certs());
    }

    #[test]
    fn client_certificate_without_key_is_rejected() {
        let config = MqttConfig { client_cert_file: Some(fixture("client.crt")), ..tls_config() };
        assert!(matches!(client_config(&config), Err(TlsError::IncompleteClientAuth)));
    }

    #[test]
    fn mismatched_client_key_is_rejected() {
        let config = MqttConfig {
            client_cert_file: Some(fixture("client.crt")),
            client_key_file: Some(fixture("server.key")),
            ..tls_config()
        };
        assert!(matches!(client_config(&config), Err(TlsError::Rustls(_))));
    }

    #[test]
    fn missing_ca_file_names_the_path() {
        let config = MqttConfig { ca_file: Some(fixture("does-not-exist.crt")), ..tls_config() };
        let err = client_config(&config).unwrap_err().to_string();
        assert!(err.contains("does-not-exist.crt"), "{err}");
    }

    #[test]
    fn file_without_certificate_is_rejected() {
        let config = MqttConfig { ca_file: Some(fixture("README.md")), ..tls_config() };
        assert!(matches!(client_config(&config), Err(TlsError::Pem(_, _))));
    }
}
