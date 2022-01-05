CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' 

if [ "$EUID" -ne 0 ]
    then echo "Please run as root"
    exit 1
fi

mkdir ~/Lodestone

apt-get update
apt-get upgrade -y
wget -qO - https://www.mongodb.org/static/pgp/server-5.0.asc | sudo apt-key add -
echo "deb [ arch=amd64,arm64 ] https://repo.mongodb.org/apt/ubuntu focal/mongodb-org/5.0 multiverse" | sudo tee /etc/apt/sources.list.d/mongodb-org-5.0.list
apt-get update
apt-get install -y mongodb-org
apt-get install software-properties-common -y
add-apt-repository ppa:linuxuprising/java
echo oracle-java17-installer shared/accepted-oracle-license-v1-3 select true | sudo /usr/bin/debconf-set-selections
apt-get install oracle-java17-installer -y
apt install cpuidtool libcpuid14 libcpuid-dev

