#!/usr/bin/env bash
set -euo pipefail

EXAMPLES_DIR="$(dirname "$0")/examples"
CERTS_DIR="$EXAMPLES_DIR/certs"

mkdir -p "$CERTS_DIR"

CERT_FILE="$CERTS_DIR/cert.pem"
KEY_FILE="$CERTS_DIR/key.pem"

echo "Generating self-signed certificate for testing in $CERTS_DIR..."

# Generate private key
openssl genrsa -out "$KEY_FILE" 2048

# Generate self-signed certificate
openssl req -new -x509 -key "$KEY_FILE" -out "$CERT_FILE" -days 365 \
    -subj "/C=US/ST=Test/L=Test/O=Test/OU=Dev/CN=localhost"

echo "Created:"
echo "  - $CERT_FILE"
echo "  - $KEY_FILE"
