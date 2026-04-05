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
