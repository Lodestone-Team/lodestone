#!/bin/bash

CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' 

if [ "$EUID" -ne 0 ]
    then echo "Please run as root"
    exit 1
fi

if [ "$#" -ne 1 ]; then
    echo "Please supply a path"
    exit 1
fi


apt-get update 
apt-get upgrade -y
apt-get install debootstrap openssh-server sshd -y

printf "${CYAN}Executing in $1, this will wipe the directory, continue? [Y/n] ${NC}\n" 

read decision

if [ $decision != "Y" ] 
    then 
    printf "${RED}Aborted.\n"
    exit -1
fi

debootstrap hirsute $1

printf "${CYAN}Mounting /proc in chroot...\n"
mount -o bind /proc "$1/proc" 


printf "${CYAN}(Highly recommanded) Add a user now? [Y/n]${NC}\n" 

read decision

if [ $decision == "Y" ] 
    then 
    printf "Enter a username: " 
    read username
    printf "${CYAN}User set up will take place in chroot${NC}\n" 
    chroot $1 /bin/bash << "EOT"
    adduser $username
EOT
else 
printf "Proceeding without adding user..."
fi

chroot $1 /bin/bash << "EOT"

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
EOT



