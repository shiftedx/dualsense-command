fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(dscc_agent::serve(dscc_agent::resolve_agent_bind_addr()));
    // A stuck OS HID call cannot be cancelled. Do not wait indefinitely for it
    // after the bounded best-effort cleanup has reported its result.
    runtime.shutdown_timeout(std::time::Duration::from_millis(250));
    result
}
