**Describe the bug**

`StreamableHttpClientTransport` in `rmcp v1.3.0` exhibits significantly higher latency on subsequent tool calls compared to the Python MCP SDK client. The first call is fast (~2ms), but every subsequent call takes ~42ms — roughly 10x slower than Python's subsequent calls (~3ms). The degradation is consistent and repro across all subsequent calls.

| Client | Call 1 | Call 2 | Call 3 | Call 4 | Call 5 |
|--------|--------|--------|--------|--------|--------|
| **Rust (rmcp)** | 1.7ms | 41.2ms | 42.0ms | 42.9ms | 41.9ms |
| **Python** | 12.0ms | 3.0ms | 2.9ms | 3.3ms | 3.2ms |

**To Reproduce**

1. Start the `servers_counter_streamhttp` server from `rmcp v1.3.0`.
2. Run the Rust client:
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
3. Run the equivalent Python client:
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

Subsequent tool calls should have similar or lower latency than the first call, as the session is already established. The Python client demonstrates this pattern (12ms → 3ms). The Rust client should not regress to ~42ms on every call after the first.

**Logs**

Rust client output:
```
Call 1: 1.7ms
Call 2: 41.2ms
Call 3: 42.0ms
Call 4: 42.9ms
Call 5: 41.9ms
```

Python client output:
```
Call 1: 12.0ms
Call 2: 3.0ms
Call 3: 2.9ms
Call 4: 3.3ms
Call 5: 3.2ms
Session termination failed: 202
```

**Additional context**

- `rmcp` version: `1.3.0`
- Rust edition: `2024`
- Server: `servers_counter_streamhttp` from `rmcp v1.3.0` samples
- The issue is consistent across all post-initialization calls (not just a one-time spike)
- Possible areas to investigate: connection reuse in reqwest, SSE stream overhead, message serialization path, or internal channel/buffering behavior in the `Worker` implementation
