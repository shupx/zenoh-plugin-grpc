use clap::Parser;
use zenoh::{
    config::Config,
    internal::{plugins::PluginsManager, runtime::RuntimeBuilder},
    session::ZenohId,
};
use zenoh_plugin_grpc::GrpcPlugin;
use zenoh_plugin_trait::Plugin;

#[derive(Parser, Debug)]
#[command(name = "zenoh bridge for gRPC")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    #[arg(short, long, value_name = "HEX_STRING")]
    id: Option<String>,
    #[arg(short, long, default_value = "peer")]
    mode: String,
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,
    #[arg(short, long, value_name = "ENDPOINT")]
    listen: Vec<String>,
    #[arg(short = 'e', long, value_name = "ENDPOINT")]
    connect: Vec<String>,
    #[arg(long)]
    no_multicast_scouting: bool,
    #[arg(long, value_name = "HOST")]
    grpc_host: Option<String>,
    #[arg(long, value_name = "PORT")]
    grpc_port: Option<u16>,
    #[arg(long, value_name = "PATH")]
    grpc_uds: Option<String>,
}

fn parse_args() -> Config {
    let args = Args::parse();
    let mut config = match &args.config {
        Some(conf_file) => Config::from_file(conf_file).unwrap(),
        None => Config::default(),
    };
    if config.plugin("grpc").is_none() {
        config.insert_json5("plugins/grpc", "{}").unwrap();
    }
    if let Some(id) = &args.id {
        config.set_id(Some(id.parse::<ZenohId>().unwrap())).unwrap();
    }
    config.set_mode(Some(args.mode.parse().unwrap())).unwrap();
    if !args.connect.is_empty() {
        config
            .connect
            .endpoints
            .set(args.connect.iter().map(|p| p.parse().unwrap()).collect())
            .unwrap();
    }
    if !args.listen.is_empty() {
        config
            .listen
            .endpoints
            .set(args.listen.iter().map(|p| p.parse().unwrap()).collect())
            .unwrap();
    }
    if args.no_multicast_scouting {
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
    }
    config.adminspace.set_enabled(true).unwrap();
    config.plugins_loading.set_enabled(true).unwrap();
    if let Some(host) = args.grpc_host {
        config
            .insert_json5("plugins/grpc/host", &format!(r#""{host}""#))
            .unwrap();
    }
    if let Some(port) = args.grpc_port {
        config
            .insert_json5("plugins/grpc/port", &port.to_string())
            .unwrap();
    }
    if let Some(uds) = args.grpc_uds {
        config
            .insert_json5("plugins/grpc/uds_path", &format!(r#""{uds}""#))
            .unwrap();
    }
    config
}

#[tokio::main]
async fn main() {
    zenoh::init_log_from_env_or("z=info");
    tracing::info!("zenoh-bridge-grpc {}", GrpcPlugin::PLUGIN_LONG_VERSION);
    let config = parse_args();
    let mut plugins_mgr = PluginsManager::static_plugins_only();
    plugins_mgr.declare_static_plugin::<zenoh_plugin_grpc::GrpcPlugin, &str>("grpc", true);

    let mut runtime = match RuntimeBuilder::new(config)
        .plugins_manager(plugins_mgr)
        .build()
        .await
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("{e}. Exiting...");
            std::process::exit(-1);
        }
    };
    if let Err(e) = runtime.start().await {
        eprintln!("Failed to start Zenoh runtime: {e}. Exiting...");
        std::process::exit(-1);
    }
    futures::future::pending::<()>().await;
}
