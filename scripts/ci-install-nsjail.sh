set -e

sudo apt update -y
sudo apt install -y \
    autoconf \
    bison \
    flex \
    gcc \
    g++ \
    git \
    libprotobuf-dev \
    libnl-route-3-dev \
    libtool \
    make \
    pkg-config \
    protobuf-compiler

git clone https://github.com/google/nsjail.git -b master --depth 1

cd nsjail
make all
sudo cp ./nsjail /usr/local/bin/nsjail
cd ..

