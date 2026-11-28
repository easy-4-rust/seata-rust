"""Create disposable MySQL TLS certificates for the owned Docker fixture."""

from datetime import datetime, timedelta, timezone
from ipaddress import ip_address
from pathlib import Path
import subprocess
import sys

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID


def certificate(name, key, issuer=None, signing_key=None):
    now = datetime.now(timezone.utc)
    subject = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, name)])
    ca = issuer is None
    builder = (x509.CertificateBuilder()
               .subject_name(subject)
               .issuer_name(subject if ca else issuer.subject)
               .public_key(key.public_key())
               .serial_number(x509.random_serial_number())
               .not_valid_before(now - timedelta(minutes=5))
               .not_valid_after(now + timedelta(hours=2))
               .add_extension(x509.BasicConstraints(ca=ca, path_length=0 if ca else None), critical=True))
    if not ca:
        builder = (builder.add_extension(x509.SubjectAlternativeName([x509.IPAddress(ip_address("127.0.0.1"))]),
                                         critical=False)
                   .add_extension(x509.ExtendedKeyUsage([ExtendedKeyUsageOID.SERVER_AUTH]), critical=False))
    return builder.sign(key if ca else signing_key, hashes.SHA256())


def new_key():
    return rsa.generate_private_key(public_exponent=65537, key_size=2048)


def generate(folder, keytool):
    folder.mkdir()
    ca_key = new_key()
    ca = certificate("Seata fixture trusted CA", ca_key)
    wrong_ca = certificate("Seata fixture unrelated CA", new_key())
    server_key = new_key()
    server = certificate("127.0.0.1", server_key, ca, ca_key)
    for name, cert in (("ca.pem", ca), ("wrong-ca.pem", wrong_ca), ("server.pem", server)):
        (folder / name).write_bytes(cert.public_bytes(serialization.Encoding.PEM))
    (folder / "server-key.pem").write_bytes(server_key.private_bytes(
        serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
    subprocess.run([keytool, "-importcert", "-noprompt", "-alias", "fixture-ca", "-file", folder / "ca.pem",
                    "-keystore", folder / "trust.p12", "-storetype", "PKCS12", "-storepass", "seata-tls-fixture"],
                   check=True, timeout=15, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)


if __name__ == "__main__":
    generate(Path(sys.argv[1]), sys.argv[2])
