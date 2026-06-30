[Read Me](README.md) > Kafka Roadmap

# Kafka Roadmap

What is needed to run `rust-web-server` alongside Apache Kafka in a microservice environment. Items are ordered by dependency — the first two unlock everything else. All Kafka I/O is handled by the [`rdkafka`](https://crates.io/crates/rdkafka) crate (librdkafka bindings; mature, production-proven).

---

## Foundation — required before anything else

### 1. Kafka connection config via env vars

There is no `RWS_CONFIG_KAFKA_*` config surface. Broker address, client ID, consumer group, security protocol, and TLS material are all unknown to the server at startup. Without this, every Kafka-integrated binary hard-codes its connection details.

**Target env vars:**

```
RWS_CONFIG_KAFKA_BROKERS=broker1:9092,broker2:9092
RWS_CONFIG_KAFKA_CLIENT_ID=my-service
RWS_CONFIG_KAFKA_CONSUMER_GROUP=my-service-group
RWS_CONFIG_KAFKA_SECURITY_PROTOCOL=SASL_SSL        # PLAINTEXT | SSL | SASL_PLAINTEXT | SASL_SSL
RWS_CONFIG_KAFKA_SASL_MECHANISM=SCRAM-SHA-256      # PLAIN | SCRAM-SHA-256 | SCRAM-SHA-512
RWS_CONFIG_KAFKA_SASL_USERNAME=my-user
RWS_CONFIG_KAFKA_SASL_PASSWORD=my-password
RWS_CONFIG_KAFKA_SSL_CA_FILE=/etc/kafka/ca.pem
RWS_CONFIG_KAFKA_SESSION_TIMEOUT_MS=30000
RWS_CONFIG_KAFKA_AUTO_OFFSET_RESET=earliest        # earliest | latest
```

These should follow the same config-layering pattern as other `RWS_CONFIG_*` values: defaults → `rws.config.toml` → env vars → CLI flags.

**Target `rws.config.toml` section:**

```toml
[kafka]
brokers = "broker1:9092,broker2:9092"
client_id = "my-service"
consumer_group = "my-service-group"
security_protocol = "SASL_SSL"
sasl_mechanism = "SCRAM-SHA-256"
sasl_username = "my-user"
sasl_password = "my-password"
```

---

### 2. Kafka producer — publish events from request handlers

There is no way to produce a Kafka message from inside a route handler. An HTTP POST that creates a resource typically needs to emit a `resource.created` event for downstream consumers. Without a producer, the service cannot participate in an event-driven architecture.

**Target API:**

```rust
use rust_web_server::kafka::{KafkaProducer, ProducerConfig};
use rust_web_server::app::App;

struct State {
    producer: KafkaProducer,
}

let producer = KafkaProducer::from_env()?;  // reads RWS_CONFIG_KAFKA_* env vars

let app = App::with_state(State { producer })
    .post("/orders", |req, _params, _conn, state| {
        // ... create order in DB ...

        state.producer.send("orders.created", r#"{"order_id": 1}"#)?;
        // or with a key for partition affinity:
        state.producer.send_keyed("orders.created", "order-1", r#"{"order_id": 1}"#)?;

        json_ok(r#"{"status": "created"}"#)
    });
```

- `KafkaProducer::from_env()` — constructs producer from `RWS_CONFIG_KAFKA_*` variables
- `KafkaProducer::new(config)` — explicit config
- `producer.send(topic, payload)` — fire-and-forget publish (async confirm via callback)
- `producer.send_keyed(topic, key, payload)` — deterministic partition routing by key
- `producer.send_sync(topic, payload)` — block until broker ACK (use sparingly; adds latency)
- `producer.send_with_headers(topic, key, payload, headers)` — attach Kafka record headers (trace IDs, schema version, correlation ID)

Requires the `kafka` Cargo feature:
```toml
rust-web-server = { version = "17", features = ["kafka"] }
```

---

## Consumers — processing messages from topics

### 3. Kafka consumer background worker

There is no background consumer loop. A service that must process incoming Kafka events (e.g., fulfil orders, update projections, invalidate caches) has no framework support — the developer must wire up `rdkafka` entirely from scratch alongside the HTTP server.

**Target API:**

```rust
use rust_web_server::kafka::{KafkaConsumer, ConsumerContext};

let consumer = KafkaConsumer::from_env()?
    .subscribe(["orders.created", "payments.completed"])?;

// Runs in its own OS thread; does not block the HTTP thread pool.
let handle = consumer.start(|msg: ConsumerContext| {
    println!("topic={} partition={} offset={}", msg.topic(), msg.partition(), msg.offset());
    let payload = msg.payload_str()?;
    // ... process ...
    msg.commit()?;   // manual offset commit after successful processing
    Ok(())
});

// Start the HTTP server alongside the consumer:
server.run();
handle.join();
```

- `KafkaConsumer::from_env()` — reads `RWS_CONFIG_KAFKA_*`
- `consumer.subscribe(topics)` — join a consumer group and subscribe to topics
- `consumer.start(handler)` — spawn a thread; returns a `ConsumerHandle` for shutdown
- `ConsumerContext::payload_str()` — decode payload as UTF-8
- `ConsumerContext::commit()` — commit offset for this partition after successful processing
- `ConsumerHandle::stop()` — initiate clean shutdown; drains in-flight messages

---

### 4. Consumer group partition assignment and rebalance hooks

Consumer group rebalances (scale-out, pod restart) can cause duplicate processing or offset loss if the service does not react to partition assignment changes. There is no hook to flush state, commit offsets, or drain in-flight work before partitions are revoked.

**Target API:**

```rust
let consumer = KafkaConsumer::from_env()?
    .on_assign(|partitions| {
        println!("assigned: {:?}", partitions);
        // warm up per-partition state
    })
    .on_revoke(|partitions| {
        println!("revoking: {:?}", partitions);
        // flush in-flight work, commit offsets before returning
    })
    .subscribe(["events.v1"])?;
```

---

### 5. Dead letter queue (DLQ)

Messages that fail processing (deserialization error, downstream timeout, business rule violation) are silently dropped or block the partition indefinitely. A DLQ routes failed messages to a separate topic for later inspection and replay without stalling the consumer.

**Target API:**

```rust
let consumer = KafkaConsumer::from_env()?
    .dead_letter_queue("orders.created.dlq")   // topic to route failures to
    .max_retries(3)                             // retry before sending to DLQ
    .retry_backoff_ms(1000)
    .subscribe(["orders.created"])?;

consumer.start(|msg| {
    process(msg.payload_str()?)?;   // on Err after max_retries → published to DLQ
    msg.commit()
});
```

DLQ records carry the original topic, partition, offset, and error reason as Kafka record headers so failures are traceable.

---

## Reliability

### 6. Transactional outbox pattern

Publishing a Kafka event and committing a database write must succeed or fail together. Doing them as two independent operations risks:
- Event published but DB write failed → phantom event
- DB committed but broker unreachable → lost event

The transactional outbox writes the event to a DB table inside the same transaction as the business data, then a background relay reads the outbox and publishes to Kafka.

**Target API:**

```rust
use rust_web_server::kafka::outbox::{Outbox, OutboxRelay};

// In the handler (inside a DB transaction):
let outbox = Outbox::new(&mut tx);
outbox.enqueue("orders.created", r#"{"order_id": 1}"#)?;
tx.commit()?;

// Background relay (started once at startup):
let relay = OutboxRelay::from_env(producer, db_pool)?;
relay.start(poll_interval_ms: 500);
```

The relay uses `SELECT … FOR UPDATE SKIP LOCKED` (Postgres) or an equivalent to avoid double-publish across multiple server instances.

---

### 7. Idempotent consumer / exactly-once semantics

At-least-once delivery means consumers may see duplicate messages (rebalances, retries, pod restarts). Handlers that are not idempotent (e.g., they increment a counter or charge a card) must deduplicate. There is no deduplication helper.

**Target API:**

```rust
use rust_web_server::kafka::dedup::MessageDeduplicator;

let dedup = MessageDeduplicator::from_db(db_pool)?;  // stores seen offsets in DB

consumer.start(|msg| {
    if dedup.is_duplicate(&msg)? {
        msg.commit()?;
        return Ok(());
    }
    process(msg.payload_str()?)?;
    dedup.mark_seen(&msg)?;
    msg.commit()
});
```

---

## Observability

### 8. Health check integration with Kafka

`/healthz` and `/readyz` currently report only HTTP server state. In a Kafka-dependent service, a pod may be `200 OK` on `/healthz` while the broker is unreachable and the consumer has been stalled for minutes. Kubernetes will route traffic to a pod that cannot process messages.

**Target behaviour:**

- `/healthz` — remains a liveness probe (always 200; reboots the pod if 503)
- `/readyz` — becomes unhealthy (`503`) if:
  - Kafka broker is unreachable (producer metadata fetch fails)
  - Consumer has not committed an offset in `RWS_CONFIG_KAFKA_READYZ_LAG_TIMEOUT_MS` (default 60 000)
  - Consumer group is in the middle of a rebalance

```
GET /readyz
{"status":"ok","kafka_producer":"ok","kafka_consumer_lag":42}

GET /readyz   (broker down)
HTTP 503
{"status":"degraded","kafka_producer":"unreachable","kafka_consumer_lag":null}
```

---

### 9. Kafka-specific metrics at `/metrics`

`/metrics` currently exports only HTTP server counters (`rws_requests_total`, `rws_errors_total`, `rws_active_connections`). There are no Kafka metrics, so operators have no visibility into throughput, lag, or error rates without a separate Kafka exporter sidecar.

**Target Prometheus metrics:**

```
# HELP rws_kafka_messages_produced_total Total messages successfully published to Kafka
# TYPE rws_kafka_messages_produced_total counter
rws_kafka_messages_produced_total{topic="orders.created"} 1024

# HELP rws_kafka_produce_errors_total Messages that failed to publish
# TYPE rws_kafka_produce_errors_total counter
rws_kafka_produce_errors_total{topic="orders.created"} 3

# HELP rws_kafka_produce_latency_ms Publish round-trip latency to broker
# TYPE rws_kafka_produce_latency_ms histogram
rws_kafka_produce_latency_ms_bucket{topic="orders.created",le="10"} 980
rws_kafka_produce_latency_ms_bucket{topic="orders.created",le="50"} 1020
rws_kafka_produce_latency_ms_bucket{topic="orders.created",le="+Inf"} 1024

# HELP rws_kafka_messages_consumed_total Total messages processed by the consumer
# TYPE rws_kafka_messages_consumed_total counter
rws_kafka_messages_consumed_total{topic="orders.created",group="my-service"} 998

# HELP rws_kafka_consumer_errors_total Messages that failed processing (before DLQ)
# TYPE rws_kafka_consumer_errors_total counter
rws_kafka_consumer_errors_total{topic="orders.created",group="my-service"} 2

# HELP rws_kafka_consumer_lag Current consumer lag (messages behind the latest offset)
# TYPE rws_kafka_consumer_lag gauge
rws_kafka_consumer_lag{topic="orders.created",partition="0",group="my-service"} 0
rws_kafka_consumer_lag{topic="orders.created",partition="1",group="my-service"} 5
```

---

## Message schema

### 10. Message serialization — JSON, Avro, Protobuf

Kafka messages are raw bytes. There is no typed serialization/deserialization layer — every handler calls `.payload_str()` and parses manually. For large teams, schema drift between producer and consumer versions causes silent data corruption.

**Target API:**

```rust
use rust_web_server::kafka::schema::{JsonMessage, AvroMessage};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct OrderCreated { order_id: u64, user_id: u64, amount_cents: u64 }

// JSON (no registry needed):
producer.send_typed("orders.created", &OrderCreated { order_id: 1, user_id: 42, amount_cents: 9900 })?;

consumer.start(|msg| {
    let event: OrderCreated = msg.deserialize_json()?;
    println!("order {} for user {}", event.order_id, event.user_id);
    msg.commit()
});

// Avro (requires schema registry — see Item 11):
producer.send_avro("orders.created", &event, schema_registry)?;
```

Requires `features = ["kafka", "serde"]` for JSON; `features = ["kafka", "avro"]` for Avro.

---

### 11. Schema registry integration (Confluent / AWS Glue)

Without a schema registry, there is no enforcement that producers and consumers agree on message structure. Field renames or type changes silently break consumers in production.

**Target API:**

```rust
use rust_web_server::kafka::schema_registry::SchemaRegistry;

let registry = SchemaRegistry::confluent("https://schema-registry:8081")
    .with_basic_auth("user", "password");

// Producer auto-registers schema on first publish and prepends schema ID:
producer.with_registry(registry.clone())
    .send_avro("orders.created", &event)?;

// Consumer looks up schema by ID embedded in the message:
consumer.with_registry(registry)
    .start(|msg| {
        let event: OrderCreated = msg.deserialize_avro()?;
        // ...
    });
```

Supports Confluent Schema Registry (`confluent`) and AWS Glue Schema Registry (`glue`).

---

## Summary

| # | Item | Status |
|---|------|--------|
| 1 | Kafka connection config via env vars | Pending |
| 2 | Kafka producer — publish events from handlers | Pending |
| 3 | Kafka consumer background worker | Pending |
| 4 | Consumer group rebalance hooks | Pending |
| 5 | Dead letter queue (DLQ) | Pending |
| 6 | Transactional outbox pattern | Pending |
| 7 | Idempotent consumer / deduplication | Pending |
| 8 | Health check integration with Kafka | Pending |
| 9 | Kafka-specific metrics at `/metrics` | Pending |
| 10 | Message serialization (JSON, Avro, Protobuf) | Pending |
| 11 | Schema registry integration | Pending |
