use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub type DynRead = Box<dyn AsyncRead + Send + Unpin>;
pub type DynWrite = Box<dyn AsyncWrite + Send + Unpin>;

/// Build a TLS acceptor from PEM cert and key files.
pub fn build_server_tls(cert_path: &str, key_path: &str) -> Result<TlsAcceptor> {
    let certs = load_certs(Path::new(cert_path))?;
    let key = load_key(Path::new(key_path))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to build TLS server config")?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Build a TLS connector for clients.
pub fn build_client_tls(
    ca_cert_path: Option<&str>,
    skip_verify: bool,
) -> Result<TlsConnector> {
    let config = if skip_verify {
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth()
    } else if let Some(ca_path) = ca_cert_path {
        let ca_certs = load_certs(Path::new(ca_path))?;
        let mut root_store = rustls::RootCertStore::empty();
        for cert in ca_certs {
            root_store
                .add(cert)
                .context("Failed to add CA certificate")?;
        }
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    } else {
        anyhow::bail!("TLS enabled but no --ca-cert provided and --tls-skip-verify not set");
    };

    Ok(TlsConnector::from(Arc::new(config)))
}

/// Generate a self-signed certificate using rcgen.
/// Returns (cert_pem, key_pem).
pub fn generate_self_signed_cert() -> Result<(String, String)> {
    let cert = rcgen::generate_simple_self_signed(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "0.0.0.0".to_string(),
    ])
    .context("Failed to generate self-signed certificate")?;

    let cert_pem = cert.cert.pem();
    let key_pem = cert.key_pair.serialize_pem();

    Ok((cert_pem, key_pem))
}

/// Build a TLS acceptor from PEM strings (for auto-generated certs).
pub fn build_server_tls_from_pem(cert_pem: &str, key_pem: &str) -> Result<TlsAcceptor> {
    let certs = load_certs_from_pem(cert_pem)?;
    let key = load_key_from_pem(key_pem)?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to build TLS server config from PEM")?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Parse server name from address string for TLS client connection.
pub fn parse_server_name(addr: &str) -> Result<ServerName<'static>> {
    // Try as IP address first
    let ip_str = addr.split(':').next().unwrap_or(addr);
    if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
        return Ok(ServerName::IpAddress(ip.into()));
    }
    // Otherwise treat as DNS name
    Ok(ServerName::try_from(ip_str.to_string())
        .context("Invalid server name for TLS")?)
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open cert file: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to parse certificates")
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open key file: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| anyhow::anyhow!("No private key found in {}", path.display()))
}

fn load_certs_from_pem(pem: &str) -> Result<Vec<CertificateDer<'static>>> {
    let mut reader = BufReader::new(pem.as_bytes());
    rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to parse PEM certificates")
}

fn load_key_from_pem(pem: &str) -> Result<PrivateKeyDer<'static>> {
    let mut reader = BufReader::new(pem.as_bytes());
    rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| anyhow::anyhow!("No private key found in PEM data"))
}

/// A certificate verifier that accepts any certificate (for --tls-skip-verify).
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dcs: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dcs: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
