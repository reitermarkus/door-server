require 'securerandom'
require 'shellwords'

TARGET = ENV['TARGET'] || 'arm-unknown-linux-gnueabi'

RPI = ENV['RPI'] || 'door-server.local'
HOST = "pi@#{RPI}"

def ssh(*args)
  sh 'ssh', HOST, *args
end

desc 'compile binary'
task :build do
  sh 'cross', 'build', '--release', '--target', TARGET
end

desc 'set time zone on Raspberry Pi'
task :setup_timezone do
  ssh 'sudo', 'timedatectl', 'set-timezone', 'Europe/Vienna'
end

desc 'set hostname on Raspberry Pi'
task :setup_hostname do
  ssh <<~SH
    if ! dpkg -s dnsutils >/dev/null; then
      sudo apt-get update
      sudo apt-get install -y dnsutils
    fi

    hostname="$(dig -4 +short -x "$(hostname -I | awk '{print $1}')")"
    hostname="${hostname%%.local.}"

    if [ -n "${hostname}" ]; then
      echo "${hostname}" | sudo tee /etc/hostname >/dev/null
    fi
  SH
end

desc 'set up I2C on Raspberry Pi'
task :setup_i2c do
  ssh 'sudo', 'raspi-config', 'nonint', 'do_i2c', '0'

  r, w = IO.pipe

  w.puts <<~CFG
    SUBSYSTEM=="i2c-dev", ATTR{name}=="bcm2835 I2C adapter", SYMLINK+="i2c", TAG+="systemd"
  CFG
  w.close

  ssh 'sudo', 'tee', '/lib/udev/rules.d/99-i2c.rules', in: r
end

desc 'set up watchdog on Raspberry Pi'
task :setup_watchdog do
  ssh <<~SH
    if ! dpkg -s watchdog >/dev/null; then
      sudo apt-get update
      sudo apt-get install -y watchdog
    fi
  SH

  r, w = IO.pipe

  w.puts 'bcm2835_wdt'
  w.close

  ssh 'sudo', 'tee', '/etc/modules-load.d/bcm2835_wdt.conf', in: r

  gateway_ip = %x(#{['ssh', HOST, 'ip', 'route'].shelljoin})[/via (\d+.\d+.\d+.\d+) /, 1]

  raise if gateway_ip.empty?

  r, w = IO.pipe

  w.puts <<~CFG
    watchdog-device	= /dev/watchdog
    ping = #{gateway_ip}
  CFG
  w.close

  ssh 'sudo', 'tee', '/etc/watchdog.conf', in: r
  ssh 'sudo', 'systemctl', 'enable', 'watchdog'
end

task :setup => [:setup_timezone, :setup_hostname, :setup_i2c, :setup_watchdog, :setup_mjpeg_streamer, :setup_shairport, :setup_soundcard]

task :install => :build do
  sh 'rsync', '-z', '--rsync-path', 'sudo rsync', "target/#{TARGET}/release/door-server", "#{HOST}:/usr/local/bin/door-server"
end

desc 'deploy binary and service configuration to Raspberry Pi'
task :deploy => :install  do
  IO.pipe do |r, w|

    w.puts <<~CFG
      [Unit]
      # StartLimitAction=reboot
      StartLimitIntervalSec=60
      StartLimitBurst=10
      Description=door-server

      [Service]
      Type=simple
      Environment=RUST_LOG=info
      ExecStart=/usr/local/bin/door-server
      Restart=always
      RestartSec=1

      [Install]
      WantedBy=multi-user.target
    CFG
    w.close

    ssh 'sudo', 'tee', '/etc/systemd/system/door-server.service', in: r
    ssh 'sudo', 'systemctl', 'enable', 'door-server'
    ssh 'sudo', 'systemctl', 'restart', 'door-server'
  end
end

desc 'show service log'
task :log do
  ssh '-t', 'journalctl', '-f', '-u', 'door-server'
end


desc 'run the application for debugging'
task :run => :install do
  ssh 'killall door-server' rescue nil
  ssh '-t', 'RUST_LOG=info /usr/local/bin/door-server'
end
