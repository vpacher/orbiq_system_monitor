#!/bin/sh
# Package install/upgrade/purge tests. Run by test-package.sh inside a throwaway
# debian:bookworm container (no systemd running). Expects /t/new.deb (this
# build) and /t/old.deb (0.3.0, the last release before the debconf setup).
set -u
export DEBIAN_FRONTEND=noninteractive
C=/etc/orbiq_system_monitor/config.toml
PASS=0; FAIL=0
ok()   { echo "  PASS: $*"; PASS=$((PASS+1)); }
bad()  { echo "  FAIL: $*"; FAIL=$((FAIL+1)); }
check(){ if eval "$1"; then ok "$2"; else bad "$2"; fi; }
reset(){ dpkg --purge orbiq-system-monitor >/dev/null 2>&1; rm -rf /etc/orbiq_system_monitor /secrets; }
toml_ok(){ python3 -c 'import tomllib,sys; tomllib.load(open(sys.argv[1],"rb"))' "$1" 2>/dev/null; }
mode(){ stat -c %a "$1"; }

apt-get -qq update >/dev/null && apt-get -qq install -y debconf-utils python3 >/dev/null 2>&1

echo "1) fresh install, nothing preseeded"
reset
dpkg -i /t/new.deb >/tmp/o 2>&1; rc=$?
check '[ $rc -eq 0 ]' "dpkg succeeds"
check '[ ! -e $C ]' "no config written"
check '[ ! -e /etc/systemd/system/multi-user.target.wants/orbiq_system_monitor.service ]' "service not enabled"
check 'grep -q "dpkg-reconfigure" /tmp/o' "tells the user how to configure"

echo "2) fresh install, preseeded (password with quote and backslash, bad device name)"
reset
debconf-set-selections <<'EOF'
orbiq-system-monitor orbiq-system-monitor/mqtt_broker string broker.example
orbiq-system-monitor orbiq-system-monitor/mqtt_port string 1884
orbiq-system-monitor orbiq-system-monitor/mqtt_username string ha
orbiq-system-monitor orbiq-system-monitor/mqtt_password password p"w\x
orbiq-system-monitor orbiq-system-monitor/device_name string my box
orbiq-system-monitor orbiq-system-monitor/enable_service boolean true
EOF
dpkg -i /t/new.deb >/tmp/o 2>&1; rc=$?
check '[ $rc -eq 0 ]' "dpkg succeeds"
check '[ -f $C ] && [ "$(mode $C)" = 600 ]' "config written, mode 600"
check 'toml_ok $C' "config is valid TOML"
check 'python3 -c "import tomllib; c=tomllib.load(open(\"$C\",\"rb\")); assert c[\"mqtt\"][\"password\"]==\"p\\\"w\\\\x\", c; assert c[\"mqtt\"][\"port\"]==1884; assert c[\"device\"][\"name\"]==\"my_box\"; assert c[\"mqtt\"][\"broker\"]==\"broker.example\""' "values round-trip (password escaped, device sanitised)"
check '[ -z "$(echo get orbiq-system-monitor/mqtt_password | debconf-communicate orbiq-system-monitor | cut -d" " -f2-)" ]' "password cleared from debconf db"
check '[ -L /etc/systemd/system/multi-user.target.wants/orbiq_system_monitor.service ]' "service enabled for boot"

echo "3) upgrade over a hand-written config (mode 644): untouched, tightened to 600"
reset
mkdir -p /etc/orbiq_system_monitor
printf '[mqtt]\nbroker = "mine"\n[device]\nname = "keep"\n' > $C; chmod 644 $C
sum_before=$(md5sum < $C)
debconf-set-selections <<'EOF'
orbiq-system-monitor orbiq-system-monitor/mqtt_broker string SHOULD_NOT_BE_USED
EOF
dpkg -i /t/old.deb >/dev/null 2>&1 </dev/null
dpkg --force-confold -i /t/new.deb >/tmp/o 2>&1; rc=$?
check '[ $rc -eq 0 ]' "dpkg succeeds"
check '[ "$(md5sum < $C)" = "$sum_before" ]' "content unchanged"
check '[ "$(mode $C)" = 600 ]' "mode tightened to 600"
check '[ ! -e /etc/orbiq_system_monitor/.configured ]' "old marker file removed"

echo "4) upgrade with config as a symlink (provisioned, e.g. encrypted volume)"
reset
mkdir -p /secrets /etc/orbiq_system_monitor
printf '[mqtt]\nbroker = "vault"\n[device]\nname = "kuma"\n' > /secrets/config.toml; chmod 640 /secrets/config.toml
ln -s /secrets/config.toml $C
dpkg --force-confold -i /t/new.deb >/tmp/o 2>&1; rc=$?
check '[ $rc -eq 0 ]' "dpkg succeeds"
check '[ -L $C ] && [ "$(readlink $C)" = /secrets/config.toml ]' "symlink kept"
check '[ "$(mode /secrets/config.toml)" = 640 ]' "target permissions untouched"
check 'grep -q vault /secrets/config.toml' "target content untouched"

echo "5) upgrade from 0.3.0 that was never configured (placeholder example) + preseed"
reset
dpkg -i /t/old.deb >/dev/null 2>&1 </dev/null
check 'grep -q __MQTT_BROKER__ $C' "0.3.0 left the placeholder example"
debconf-set-selections <<'EOF'
orbiq-system-monitor orbiq-system-monitor/mqtt_broker string 10.0.0.2
orbiq-system-monitor orbiq-system-monitor/mqtt_username string
orbiq-system-monitor orbiq-system-monitor/enable_service boolean false
EOF
dpkg --force-confold -i /t/new.deb >/tmp/o 2>&1; rc=$?
check '[ $rc -eq 0 ]' "dpkg succeeds"
check 'grep -q "broker = \"10.0.0.2\"" $C && ! grep -q __MQTT $C' "placeholder config replaced"
check '! grep -q "^username" $C' "no credentials written for anonymous broker"
check '[ ! -e /etc/systemd/system/multi-user.target.wants/orbiq_system_monitor.service ]' "not enabled (answered no)"

echo "6) purge"
dpkg --purge orbiq-system-monitor >/tmp/o 2>&1; rc=$?
check '[ $rc -eq 0 ]' "purge succeeds"
check '[ ! -e /etc/orbiq_system_monitor ]' "config directory removed"
check '[ -z "$(debconf-show orbiq-system-monitor 2>/dev/null)" ]' "debconf answers purged"

echo "7) interactive (teletype frontend, answers typed on stdin)"
reset
printf 'broker.lan\n\nalice\nsecret\n\nyes\n' | DEBIAN_FRONTEND=teletype dpkg -i /t/new.deb >/tmp/o 2>&1; rc=$?
check '[ $rc -eq 0 ]' "dpkg succeeds"
check 'grep -q "broker = \"broker.lan\"" $C && grep -q "port = 1883" $C' "broker typed, port default 1883 kept"
check 'grep -q "username = \"alice\"" $C && grep -q "password = \"secret\"" $C' "credentials typed"
check 'grep -q "name = \"$(hostname -s | tr -c "A-Za-z0-9_\n-" _)\"" $C' "device name defaults to the hostname"
grep -E "MQTT|Device|Start" /tmp/o | head -8 | sed 's/^/    prompt: /'

echo "RESULT: $PASS passed, $FAIL failed"
