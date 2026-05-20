#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dscc_agent::serve(dscc_agent::resolve_agent_bind_addr()).await
}
