use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ServiceExt, model::*};
use std::{env, time::Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let (url, tool) = (&args[1], &args[2]);

    let transport = StreamableHttpClientTransport::from_uri(url.clone());
    let client = ClientInfo::default().serve(transport).await?;

    for i in 1..=5 {
        let start = Instant::now();
        let _res = client
            .call_tool(CallToolRequestParams::new(tool.clone()))
            .await
            .unwrap_or_default();

        println!(
            "Call {}: {:.1}ms",
            i,
            start.elapsed().as_secs_f32() * 1000.
        );
    }

    client.cancel().await?;
    Ok(())
}
