// Copyright 2017 Hove and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

use chrono::{DateTime, FixedOffset};
use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt as _, prelude::*};
use transit_model::{transfers::generates_transfers, Result};

use opentelemetry::{global, trace::TracerProvider};
// use opentelemetry::trace::{TracerProvider as _};
use opentelemetry_sdk::trace::{SdkTracer, SdkTracerProvider};
// use tonic::transport::ClientTlsConfig;

use opentelemetry_otlp::{SpanExporter as OtlmSpanExporter, WithExportConfig};

lazy_static::lazy_static! {
    pub static ref GIT_VERSION: String = transit_model::binary_full_version(env!("CARGO_PKG_VERSION"));

    static ref OTLP_ENDPOINT: String =
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").expect("OTEL_EXPORTER_OTLP_ENDPOINT not set");
}

fn get_version() -> &'static str {
    &GIT_VERSION
}

#[derive(Debug, Parser)]
#[command(name = "ntfs2ntfs", about = "Convert an NTFS to an NTFS.", version = get_version())]
struct Opt {
    /// Input directory.
    #[arg(short = 'i', long = "input", default_value = ".")]
    input: PathBuf,

    /// Output directory.
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Current datetime.
    #[arg(
        short = 'x',
        long,
        default_value = &**transit_model::CURRENT_DATETIME
    )]
    current_datetime: DateTime<FixedOffset>,

    /// The maximum distance in meters to compute the tranfer.
    #[arg(long, short = 'd', default_value = transit_model::TRANSFER_MAX_DISTANCE)]
    max_distance: f64,

    /// The walking speed in meters per second. You may want to divide your
    /// initial speed by sqrt(2) to simulate Manhattan distances.
    #[arg(long, short = 's', default_value = transit_model::TRANSFER_WALKING_SPEED)]
    walking_speed: f64,

    /// Waiting time at stop in seconds.
    #[arg(long, short = 't', default_value = transit_model::TRANSFER_WAITING_TIME)]
    waiting_time: u32,

    /// Don't compute transfers even the transfers of the stop point to itself (max_distance = 0.0)
    #[arg(long)]
    ignore_transfers: bool,
}

fn init_tracer_provider() -> SdkTracerProvider {
    let otlp_endpoint = OTLP_ENDPOINT.clone();
    let otlp_exporter = OtlmSpanExporter::builder()
        .with_tonic()
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_endpoint(otlp_endpoint)
        // .with_tls_config(ClientTlsConfig::new().with_native_roots())
        .build()
        .expect("Failed to create OTLP exporter");

    let provider = SdkTracerProvider::builder()
        // .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .with_batch_exporter(otlp_exporter)
        .build();

    global::set_tracer_provider(provider.clone());

    provider
}

fn init_tracing_subscriber(tracer: SdkTracer) {
    let filter = EnvFilter::new(
        "info,opentelemetry_rust_demo=debug,opentelemetry_sdk=warn,opentelemetry_otlp=warn,opentelemetry_http=warn,reqwest=warn,hyper_util=warn,hyper=warn,h2=warn,tonic=warn",
    );

    let otel_span_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(otel_span_layer)
        .init();
}

fn run(opt: Opt) -> Result<()> {
    info!("Launching ntfs2ntfs...");

    let model = transit_model::ntfs::read(opt.input)?;
    let model = if opt.ignore_transfers {
        model
    } else {
        let collections = generates_transfers(
            model,
            opt.max_distance,
            opt.walking_speed,
            opt.waiting_time,
            None,
        )?;
        transit_model::Model::new(collections)?
    };

    if let Some(output) = opt.output {
        match output.extension() {
            Some(ext) if ext == "zip" => {
                transit_model::ntfs::write_to_zip(&model, output, opt.current_datetime)?;
            }
            _ => {
                transit_model::ntfs::write(&model, output, opt.current_datetime)?;
            }
        };
    }
    Ok(())
}

#[tokio::main]

async fn main() {
    let tracer_provider = init_tracer_provider();
    let tracing_layer_tracer = tracer_provider.tracer("transit-model");
    init_tracing_subscriber(tracing_layer_tracer);
    if let Err(err) = run(Opt::parse()) {
        for cause in err.chain() {
            eprintln!("{cause}");
        }
        std::process::exit(1);
    }
    tracer_provider
        .shutdown()
        .expect("Failed to shutdown tracer provider");
}
