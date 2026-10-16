use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair, KeyUsagePurpose, SanType};
use std::net::IpAddr;
use thiserror::Error;

pub struct TlsMaterial {
    pub certificate_pem: Vec<u8>,
    pub server_private_key_pem: Vec<u8>,
    pub trusted_ca_certificate_pem: Vec<u8>,
    pub ca_private_key_pem: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum ProvisionError {
    #[error("certificate generation failed: {0}")]
    Certificate(#[from] rcgen::Error),
}

/// Creates a private per-install CA and a leaf certificate for localhost and the chosen host.
pub fn provision_tls(host: IpAddr) -> Result<TlsMaterial, ProvisionError> {
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let ca_key = KeyPair::generate()?;
    let ca_certificate = ca_params.self_signed(&ca_key)?;

    let mut leaf_params = CertificateParams::default();
    leaf_params.subject_alt_names = vec![
        SanType::DnsName("localhost".try_into().expect("static DNS name is valid")),
        SanType::IpAddress(IpAddr::from([127, 0, 0, 1])),
        SanType::IpAddress(host),
    ];
    let leaf_key = KeyPair::generate()?;
    let leaf_certificate = leaf_params.signed_by(&leaf_key, &ca_certificate, &ca_key)?;

    let certificate_pem =
        format!("{}{}", leaf_certificate.pem(), ca_certificate.pem()).into_bytes();
    Ok(TlsMaterial {
        certificate_pem,
        server_private_key_pem: leaf_key.serialize_pem().into_bytes(),
        trusted_ca_certificate_pem: ca_certificate.pem().into_bytes(),
        ca_private_key_pem: ca_key.serialize_pem().into_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provisions_leaf_chain_and_private_ca_for_requested_hosts() {
        let material = provision_tls("192.168.1.20".parse().unwrap()).unwrap();
        let chain = String::from_utf8(material.certificate_pem).unwrap();
        let trust = String::from_utf8(material.trusted_ca_certificate_pem).unwrap();
        assert_eq!(chain.matches("BEGIN CERTIFICATE").count(), 2);
        assert_eq!(trust.matches("BEGIN CERTIFICATE").count(), 1);
        assert!(
            String::from_utf8(material.server_private_key_pem)
                .unwrap()
                .contains("PRIVATE KEY")
        );
        assert!(
            String::from_utf8(material.ca_private_key_pem)
                .unwrap()
                .contains("PRIVATE KEY")
        );
        assert!(chain.contains("END CERTIFICATE"));
    }
}
