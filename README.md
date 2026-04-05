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
   ```rust
   use rmcp::transport::StreamableHttpClientTransport;
   use rmcp::{ServiceExt, model::*};
   use std::time::Instant;

   #[tokio::main]
   async fn main() -> Result<(), Box<dyn std::error::Error>> {
       let transport = StreamableHttpClientTransport::from_uri("http://localhost:8000/mcp");
       let client = ClientInfo::default().serve(transport).await?;

       for i in 1..=5 {
           let start = Instant::now();
           let _res = client.call_tool(CallToolRequestParams::new("say_hello")).await.unwrap();
           println!("Call {}: {:.1}ms", i, start.elapsed().as_secs_f32() * 1000.0);
       }
       client.cancel().await?;
       Ok(())
   }
   ```
3. Run the Rust client with custom configuration (workaround):
   ```bash
   cargo run --release -- custom
   ```
   This uses a custom reqwest client with `pool_max_idle_per_host(0)` to disable connection reuse:
   ```rust
   let reqwest_client = reqwest::Client::builder()
       .pool_max_idle_per_host(0)
       .build()?;

   let transport = StreamableHttpClientTransport::with_client(
       reqwest_client,
       StreamableHttpClientTransportConfig::with_uri(url),
   );
   ```
4. Run the equivalent Python client:
   ```python
   import asyncio, time
   from mcp import ClientSession
   from mcp.client.streamable_http import streamablehttp_client

   async def main():
       async with streamablehttp_client(url="http://localhost:8000/mcp") as (read, write, _):
           async with ClientSession(read, write) as session:
               await session.initialize()
               for i in range(1, 6):
                   start = time.perf_counter()
                   await session.call_tool("say_hello", arguments={})
                   print(f"Call {i}: {(time.perf_counter() - start) * 1000:.1f}ms")

   asyncio.run(main())
   ```

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
