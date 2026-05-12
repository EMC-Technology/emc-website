use knowledge_api::application;

#[tokio::main]
async fn main() {
    if let Err(e) = application::run().await {
        eprintln!("Fatal error: {e}");
        std::process::exit(1);
    }
}
