#!/bin/sh

set -e
set +o pipefail
set +o nounset

# Turn off Wi-Fi power management.
sudo iwconfig wlan0 power off

sudo apt-get update

sudo apt-get install -y build-essential \
                        git \
                        xmltoman \
                        autoconf \
                        automake \
                        libtool \
                        libdaemon-dev \
                        libpopt-dev \
                        libconfig-dev \
                        libasound2-dev \
                        avahi-daemon \
                        libavahi-client-dev \
                        libssl-dev

# Uninstall previous version.
sudo rm -f /usr/local/bin/shairport-sync
sudo rm -f /etc/systemd/system/shairport-sync.service
sudo rm -f /etc/init.d/shairport-sync

pushd /tmp
git clone https://github.com/mikebrady/shairport-sync.git
pushd shairport-sync
autoreconf -fi
./configure --sysconfdir=/etc --with-alsa --with-avahi --with-ssl=openssl --with-systemd
make
sudo make install
popd
rm -r shairport-sync
popd

sudo systemctl enable shairport-sync
sudo systemctl start shairport-sync

if ! [ -f /etc/shairport-sync.conf.sample ]; then
  sudo cp /etc/shairport-sync.conf /etc/shairport-sync.conf.sample
fi

sudo adduser shairport-sync gpio

cat <<CONFIG | sudo tee /etc/shairport-sync.conf
general =
{
  name = "Garage";
};
CONFIG

# This is only for testing the connection triggers.
sudo apt-get install -y espeak

cat <<CONFIG | sudo tee -a /etc/shairport-sync.conf
sessioncontrol =
{
  run_this_before_play_begins = "/usr/bin/espeak -k3 'AirPlay activated.'";
  run_this_after_play_ends = "/usr/bin/espeak -k3 'AirPlay deactivated.'";
  wait_for_completion = "yes";
};
CONFIG
