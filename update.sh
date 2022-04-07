if [ -f "Lodestone" ]; then 
    rm Lodestone
    rm -r web/*
    printf "${CYAN}Starting download... ${NC}\n" 
    wget https://nightly.link/Lodestone-Team/backend/workflows/backend/main/Lodestone.zip -O lodestone.zip
    wget https://nightly.link/Lodestone-Team/frontend/workflows/frontend/main/frontend.zip -O frontend.zip
    printf "${CYAN}Download ok! ${NC}\n" 
    unzip lodestone.zip && rm lodestone.zip
    mv target/release/Lodestone .
    chmod u+x Lodestone
    rm -r target
    unzip -d web/ frontend.zip && rm frontend.zip
else 
    echo "lodestone doesn't exist, exiting..."
    exit -1
fi