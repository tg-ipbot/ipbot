#!/bin/bash

ECC_PRIVATE_KEY_NAME=private-key.pem
ECC_PRIVATE_KEY_PKCS8_NAME=private-key.pk8
ECC_CERT_NAME=cert.pem
SSL_CONF=ssl.conf

# Generate ECC private key (secp384r1)
openssl ecparam -out $ECC_PRIVATE_KEY_NAME -name secp384r1 -genkey
# Generate ECC certificate
openssl req -new -x509 -days 365 -config $SSL_CONF -key $ECC_PRIVATE_KEY_NAME -out $ECC_CERT_NAME
# Convert to PKCS8 format
openssl pkcs8 -topk8 -in $ECC_PRIVATE_KEY_NAME -out $ECC_PRIVATE_KEY_PKCS8_NAME -nocrypt
