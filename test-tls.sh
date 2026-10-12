#!/bin/bash
# End-to-end MQTT TLS test: a Mosquitto broker in a container with a TLS
# listener (8883) and a mutual-TLS listener (8884), using the test
# certificates in tests/tls. The locally built OrbIQ binary connects to it;
# mosquitto_sub inside the container checks what arrives.
# Needs podman (or CONTAINER_ENGINE=docker).
set -euo pipefail

ENGINE="${CONTAINER_ENGINE:-podman}"
CERTS="$(cd "$(dirname "$0")" && pwd)/tests/tls"
NAME="orbiq-tls-test-$$"
PASS=0
FAIL=0

work=$(mktemp -d)
cleanup() {
    "$ENGINE" rm -f "$NAME" >/dev/null 2>&1 || true
    rm -rf "$work"
}
trap cleanup EXIT

cat > "$work/mosquitto.conf" <<'EOF'
per_listener_settings true

listener 8883
cafile /certs/ca.crt
certfile /certs/server.crt
keyfile /certs/server.key
allow_anonymous true

listener 8884
cafile /certs/ca.crt
certfile /certs/server.crt
keyfile /certs/server.key
require_certificate true
allow_anonymous true
EOF

cargo build --release --quiet
BIN="$(pwd)/target/release/orbiq_system_monitor"

"$ENGINE" run -d --name "$NAME" -p 18883:8883 -p 18884:8884 \
    -v "$CERTS:/certs:ro" -v "$work/mosquitto.conf:/mosquitto/config/mosquitto.conf:ro" \
    docker.io/library/eclipse-mosquitto:2 >/dev/null
sleep 2

# run_case <name> <expect: arrives|refused> <config lines...>
run_case() {
    local name=$1 expect=$2 device=$1
    shift 2
    local dir="$work/$name"
    mkdir -p "$dir"
    {
        echo "[mqtt]"
        printf '%s\n' "$@"
        echo "[device]"
        echo "name = \"$device\""
        echo "update_interval_secs = 5"
        echo "discovery_delay_ms = 0"
    } > "$dir/config.toml"

    # Subscriber first (on the plain-TLS listener, which always works), then the daemon.
    "$ENGINE" exec "$NAME" mosquitto_sub -h localhost -p 8883 --cafile /certs/ca.crt \
        -t "homeassistant/sensor/orbiq_${device}/+/config" -C 1 -W 20 > "$dir/sub.out" 2>&1 &
    local sub=$!
    (cd "$dir" && exec "$BIN") > "$dir/daemon.log" 2>&1 &
    local daemon=$!

    local got=no
    if wait "$sub" && [ -s "$dir/sub.out" ]; then got=yes; fi
    kill "$daemon" 2>/dev/null || true
    wait "$daemon" 2>/dev/null || true

    if { [ "$expect" = arrives ] && [ $got = yes ]; } || { [ "$expect" = refused ] && [ $got = no ]; }; then
        echo "PASS: $name (sensors $([ $got = yes ] && echo arrived || echo did not arrive), as expected)"
        PASS=$((PASS + 1))
    else
        echo "FAIL: $name (expected: $expect)"
        sed 's/^/    /' "$dir/daemon.log" | tail -5
        FAIL=$((FAIL + 1))
    fi
    grep -m1 -E "MQTT Error|configuration error" "$dir/daemon.log" | sed "s/^/    daemon: /" || true
}

run_case tls_correct_ca arrives \
    'broker = "localhost"' 'port = 18883' 'tls = true' "ca_file = \"$CERTS/ca.crt\""
run_case tls_wrong_ca refused \
    'broker = "localhost"' 'port = 18883' 'tls = true' "ca_file = \"$CERTS/other-ca.crt\""
run_case mtls_with_client_cert arrives \
    'broker = "localhost"' 'port = 18884' 'tls = true' "ca_file = \"$CERTS/ca.crt\"" \
    "client_cert_file = \"$CERTS/client.crt\"" "client_key_file = \"$CERTS/client.key\""
run_case mtls_without_client_cert refused \
    'broker = "localhost"' 'port = 18884' 'tls = true' "ca_file = \"$CERTS/ca.crt\""

echo "RESULT: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
