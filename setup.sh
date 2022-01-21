#!/bin/bash

CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

PUBLIC_IP="$(dig +short myip.opendns.com @resolver1.opendns.com)"
CA_SUBJECT="/C=US/ST=CA/O=Lodestone CA/CN=Lodestone Root CA"
SUBJECT="/C=US/ST=CA/O=Rocket/CN=$PUBLIC_IP"
ALT="DNS:$PUBLIC_IP"

function gen_rsa_sha256() {
  gen_ca_if_non_existent

  openssl req -newkey rsa:4096 -nodes -sha256 -keyout rsa_sha256_key.pem \
    -subj "${SUBJECT}" -out server.csr

  openssl x509 -req -sha256 -extfile <(printf "subjectAltName=${ALT}") -days 3650 \
    -CA ca_cert.pem -CAkey ca_key.pem -CAcreateserial \
    -in server.csr -out rsa_sha256_cert.pem

  openssl pkcs12 -export -password pass:rocket \
    -in rsa_sha256_cert.pem -inkey rsa_sha256_key.pem -out rsa_sha256.p12

  rm ca_cert.srl server.csr
}

function gen_ca() {
  openssl genrsa -out ca_key.pem 4096
  openssl req -new -x509 -days 3650 -key ca_key.pem \
    -subj "${CA_SUBJECT}" -out ca_cert.pem
}

function gen_ca_if_non_existent() {
  if ! [ -f ./ca_cert.pem ]; then gen_ca; fi
}

if mkdir lodestone ; then 
    cd lodestone
    printf "${CYAN}Starting download... ${NC}\n" 
    wget https://nightly.link/CheatCod/Lodestone/workflows/rust/main/Lodestone.zip -O lodestone.zip
    wget https://nightly.link/CheatCod/Lodestone/workflows/node.js/main/frontend.zip -O frontend.zip
    printf "${CYAN}Download ok! ${NC}\n" 
    unzip lodestone.zip && rm lodestone.zip
    mv target/release/Lodestone .
    chmod u+x Lodestone
    rm -r target
    unzip -d web/ frontend.zip && rm frontend.zip

    printf "${CYAN}Setting up TLS for ${PUBLIC_IP}... ${NC}\n" 

    mkdir private
    cd private
    gen_rsa_sha256
    printf "${CYAN}TLS setup ok!...${NC}\n" 
    cd ..
    mkdir db
    mongod --fork --syslog --dbpath db
    echo "Setup success! Run ./lodestone"
else 
    echo "lodestone already exists, exiting..."
    exit -1
fi