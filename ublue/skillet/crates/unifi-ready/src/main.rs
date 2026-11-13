use clap::Parser;
use std::time::Duration;

#[derive(Parser)]
#[command(about = "Wait for the local UniFi JSON API, without logging in")]
struct Args {
    #[arg(long, default_value = "https://127.0.0.1:8443/status")]
    url: String,
    #[arg(long, default_value_t = 180)]
    timeout_secs: u64,
}

fn main() -> Result<(), skillet_unifi_ready::Error> {
    let args = Args::parse();
    skillet_unifi_ready::wait(&args.url, Duration::from_secs(args.timeout_secs))?;
    println!("UniFi JSON API is ready");
    Ok(())
}
