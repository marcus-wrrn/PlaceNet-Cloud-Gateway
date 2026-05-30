# Cloud Gateway — Operations

The gateway authenticates Hamlet servers over HTTPS and provisions them into a
Mosquitto broker (Dynamic Security plugin) so they can communicate over MQTTS.

## Components

- **`placenet-cloud-gateway serve`** — runs the HTTPS login API (`/api/login`),
  the dynsec admin MQTT client, and the legacy WebSocket relay.
- **`placenet-cloud-gateway seed-user <username> [--password ...]`** — inserts /
  updates a Hamlet login credential in the SQLite store. Omit `--password` to be
  prompted.
- **Mosquitto + dynsec** — runs in Docker; per-device clients/ACLs are created on
  demand by the gateway at login time.

## Registration flow

1. Hamlet `POST`s `{username, password}` to `https://<gateway>:8443/api/login`.
2. Gateway verifies the credential, assigns/looks up a stable `device_id`,
   generates a fresh random MQTT password, and provisions the device in dynsec
   (create-or-update: client + role + ACLs scoped to its three topics).
3. Response: `{ protocol_version, device_id, mqtt_username, mqtt_password,
   broker:{host,port}, topics:{cmds,connect,notify} }`.
4. Hamlet connects to `broker.host:broker.port` over MQTTS with the returned
   credentials and publishes an `alive` envelope on `placenet/<device_id>/notify`.

Topics (per device): `placenet/<device_id>/{cmds,connect,notify}`.

## Deploy

```sh
# 1. Provide TLS certs (Let's Encrypt or your CA):
#    certs/broker/server.crt   certs/broker/server.key      (MQTTS, port 8883)
#    certs/gateway/gateway.crt certs/gateway/gateway.key    (HTTPS API, port 8443)

# 2. Set the broker admin secret (shared by broker + gateway):
echo "DYNSEC_ADMIN_PASSWORD=$(openssl rand -hex 24)"      >  .env
echo "GATEWAY_BROKER_PUBLIC_HOST=gateway.example.com"     >> .env

# 3. Bring up the stack:
docker compose up -d --build

# 4. Seed a Hamlet credential (runs inside the gateway container):
docker compose exec gateway placenet-cloud-gateway seed-user hamlet-1
```

## Configuration (environment variables)

| Variable | Default | Description |
|---|---|---|
| `GATEWAY_API_HOST` | `0.0.0.0` | Login API bind host |
| `GATEWAY_API_PORT` | `8443` | Login API port |
| `GATEWAY_TLS_ENABLED` | `false` | Serve the API over TLS (set `true` in prod) |
| `GATEWAY_TLS_CERT` / `GATEWAY_TLS_KEY` | `certs/gateway.*` | API TLS cert/key |
| `GATEWAY_DATABASE_URL` | `sqlite://placenet_gateway.db` | Credential/device store |
| `GATEWAY_MQTT_HOST` / `GATEWAY_MQTT_PORT` | `localhost` / `1883` | Broker (dynsec admin) |
| `DYNSEC_ADMIN_USER` / `DYNSEC_ADMIN_PASSWORD` | `gateway-admin` / — | dynsec admin credential |
| `GATEWAY_BROKER_PUBLIC_HOST` | = `GATEWAY_MQTT_HOST` | MQTTS host returned to Hamlets |
| `GATEWAY_BROKER_PUBLIC_PORT` | `8883` | MQTTS port returned to Hamlets |
| `GATEWAY_PORT` | `8080` | Legacy WebSocket relay port |

## Notes

- The plain `1883` broker listener is **internal only** — never publish it to the
  host. The gateway reaches it over the Docker network for dynsec control.
- dynsec state (`dynamic-security.json`) lives on the `broker-data` volume. If that
  volume is wiped, all devices must re-login (which re-provisions them automatically).
- Default ACL access is **deny**; a device can only touch its own three topics.
