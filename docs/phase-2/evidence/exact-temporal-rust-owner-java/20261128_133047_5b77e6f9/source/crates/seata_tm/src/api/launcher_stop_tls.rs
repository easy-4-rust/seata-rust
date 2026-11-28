use ring::digest::{digest, SHA256};
use rustls::{pki_types::CertificateDer, RootCertStore};
use std::{fmt::Display, io};

pub(super) const STOP_ALPN: &[u8] = b"seata-tm-stop/1";

pub(super) fn config_error(error: impl Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}

pub(super) fn validate_trust(roots: &RootCertStore, pins: &[[u8; 32]]) -> io::Result<()> {
    if roots.is_empty() || pins.is_empty() {
        return Err(config_error(
            "TM stop TLS needs trusted roots and peer certificate pins",
        ));
    }
    Ok(())
}

pub(super) fn verify_peer(
    alpn: Option<&[u8]>,
    certificates: Option<&[CertificateDer<'_>]>,
    pins: &[[u8; 32]],
) -> io::Result<()> {
    if alpn == Some(STOP_ALPN)
        && certificates
            .and_then(|chain| chain.first())
            .is_some_and(|leaf| {
                let fingerprint = digest(&SHA256, leaf.as_ref());
                pins.iter()
                    .any(|pin| pin.as_slice() == fingerprint.as_ref())
            })
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "TM stop TLS peer certificate pin or ALPN mismatch",
    ))
}

pub(super) fn timeout_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "TM stop TLS handshake or exchange timed out",
    )
}
