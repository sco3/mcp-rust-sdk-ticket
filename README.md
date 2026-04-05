**Describe the bug**

`StreamableHttpClientTransport` in `rmcp v1.3.0` exhibits significantly higher latency on subsequent tool calls compared to the Python MCP SDK client. The first call is fast (~0.6ms), but every subsequent call takes ~41ms — roughly 10x slower than Python's subsequent calls (~3ms). The degradation is consistent and reproducible across all subsequent calls.

**Workaround**: Using a custom reqwest client with connection pooling disabled (`pool_max_idle_per_host(0)`) resolves the issue, bringing subsequent calls down to ~0.4ms.

| Client | Call 1 | Call 2 | Call 3 | Call 4 | Call 5 |
|--------|--------|--------|--------|--------|--------|
| **Rust (rmcp default)** | 0.6ms | 42.3ms | 41.0ms | 41.1ms | 41.1ms |
| **Rust (custom client)** | 0.5ms | 0.4ms | 0.4ms | 0.3ms | 0.4ms |
| **Python** | 9.5ms | 2.8ms | 2.9ms | 3.0ms | 2.6ms |

**To Reproduce**

1. Start the `servers_counter_streamhttp` server from `rmcp v1.3.0`.
2. Run the Rust client (default):
   ```bash
   cargo run --release
   ```
3. Run the Rust client with custom configuration (workaround):
   ```bash
   cargo run --release -- custom
   ```

## Full `main.rs`

This is the complete client implementation used for benchmarking. It supports both the default and custom client modes via a command-line argument:

```rust
use rmcp::transport::{
    StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
};
use rmcp::{ServiceExt, model::*};
use std::{env, time::Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "http://localhost:8000/mcp";
    let tool = "say_hello";
    let use_custom_client = env::args().any(|arg| arg == "custom");

    let client = if use_custom_client {
        let reqwest_client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()?;

        let transport = StreamableHttpClientTransport::with_client(
            reqwest_client,
            StreamableHttpClientTransportConfig::with_uri(url),
        );
        ClientInfo::default().serve(transport).await?
    } else {
        let transport = StreamableHttpClientTransport::from_uri(url);
        ClientInfo::default().serve(transport).await?
    };

    for i in 1..=5 {
        let start = Instant::now();
        let _res = client
            .call_tool(CallToolRequestParams::new(tool))
            .await
            .unwrap_or_default();

        println!(
            "Call {}: {:.1}ms",
            i,
            start.elapsed().as_secs_f32() * 1000.0
        );
    }

    client.cancel().await?;
    Ok(())
}
```

## Default vs Custom Client: What's the Difference?

The key difference lies in how the underlying `reqwest` HTTP client handles **connection pooling**:

### Default Client

```rust
let transport = StreamableHttpClientTransport::from_uri(url);
```

- `StreamableHttpClientTransport::from_uri()` creates an internal `reqwest::Client` with **default settings**.
- By default, `reqwest` enables connection pooling — idle connections are kept alive and reused for subsequent requests to the same host.
- **The problem**: In this specific scenario, reusing pooled connections introduces ~41ms of overhead on every call after the first. This may be due to the server closing the Streamable HTTP session on the reused connection, forcing a renegotiation or timeout wait before a new connection is established.

### Custom Client

```rust
let reqwest_client = reqwest::Client::builder()
    .pool_max_idle_per_host(0)  // Disable connection pooling
    .build()?;

let transport = StreamableHttpClientTransport::with_client(
    reqwest_client,
    StreamableHttpClientTransportConfig::with_uri(url),
);
```

- A `reqwest::Client` is manually constructed with `pool_max_idle_per_host(0)`, which **disables connection pooling entirely**.
- Each request opens a fresh connection, and the connection is closed immediately after use — no idle connections are retained.
- **The result**: Subsequent calls drop from ~41ms to ~0.4ms because the overhead of managing/reusing stale pooled connections is eliminated.

### Why Does This Happen?

Streamable HTTP (used by `StreamableHttpClientTransport`) establishes a session that may have specific lifecycle expectations. When a pooled connection is reused:

1. The server may have already cleaned up the previous session.
2. The client attempts to reuse the TCP connection, but the server responds with an error or forces a re-handshake.
3. This adds latency as the client falls back to establishing a new connection.

Disabling pooling ensures each call gets a clean connection, avoiding this stale-connection overhead. The trade-off is the cost of a TCP handshake on every call, but in local/benchmark scenarios this is negligible (~0.4ms).

### When to Use Each Approach

| Approach | Use When |
|----------|----------|
| **Default client** | General use cases where connection pooling benefits outweigh edge-case session lifecycle issues. |
| **Custom client (`pool_max_idle_per_host(0)`)** | When interacting with Streamable HTTP servers that exhibit session/connection reuse issues, or when consistent low-latency is critical. |

**Expected behavior**

Subsequent tool calls should have similar or lower latency than the first call, as the session is already established. The Python client demonstrates this pattern (9.5ms → 2.8ms). The Rust client should not regress to ~42ms on every call after the first. The custom client workaround shows that disabling connection pooling brings subsequent calls down to ~0.4ms, proving the issue is related to connection reuse behavior.

**Logs**

Rust client output (default):
```
Call 1: 0.6ms
Call 2: 42.3ms
Call 3: 41.0ms
Call 4: 41.1ms
Call 5: 41.1ms
```

Rust client output (custom client with `pool_max_idle_per_host(0)`):
```
Call 1: 0.5ms
Call 2: 0.4ms
Call 3: 0.4ms
Call 4: 0.3ms
Call 5: 0.4ms
```

Python client output:
```
Call 1: 9.5ms
Call 2: 2.8ms
Call 3: 2.9ms
Call 4: 3.0ms
Call 5: 2.6ms
Session termination failed: 202
```

**Additional context**

- `rmcp` version: `1.3.0`
- Rust edition: `2024`
- Server: `servers_counter_streamhttp` from `rmcp v1.3.0` samples
- The issue is consistent across all post-initialization calls (not just a one-time spike)
- Possible areas to investigate: connection reuse in reqwest, SSE stream overhead, message serialization path, or internal channel/buffering behavior in the `Worker` implementation
