
mod routes;
mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let config = config::read().await?;

    let app = routes::make_router(&config).await;

    let addr = ("0.0.0.0", 3000);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let addr = listener.local_addr()?;
    tracing::info!("starting server on http://{addr}");

    axum::serve::serve(listener, app).with_graceful_shutdown(async {
        _ = tokio::signal::ctrl_c().await;
    }).await?;

    Ok(())
}

