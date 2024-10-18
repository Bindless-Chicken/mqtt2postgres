use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;
use chrono::Utc;
use rumqttc::v5::{AsyncClient, Event, Incoming, MqttOptions};
use rumqttc::v5::mqttbytes::QoS;
use serde_json::Value;
use tracing::{debug, error, info};
use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use sqlx::{query, PgPool};

type MqttTopics = HashMap<String, Vec<String>>;

#[derive(Debug, Deserialize)]
struct FileConfig {
    redis: PostgresConfig,
    mqtt: MqttConfig,
    topics: MqttTopics,
}

#[derive(Debug, Deserialize)]
struct PostgresConfig {
    url: String,
}

#[derive(Debug, Deserialize)]
struct MqttConfig {
    host: String,
    port: u16,
    client_id: String,
}

/// mqtt2redis storing mqtt in redis
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Config file
    #[arg(
        long,
        env = "M2R_CONFIG",
        default_value = "config.toml"
    )]
    config: String,
}

fn from_file(file_path: &Path) -> Result<FileConfig> {
    let config = fs::read_to_string(file_path)?;
    Ok(toml::from_str::<FileConfig>(&config)?)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    info!("Starting Mqtt2Postgres {}", env!("CARGO_PKG_VERSION"));

    let args = Args::parse();
    let config = from_file(&Path::new(args.config.as_str())).unwrap();

    let pool = PgPool::connect(&config.redis.url).await.unwrap();

    let mut mqtt_options = MqttOptions::new(config.mqtt.client_id, config.mqtt.host, config.mqtt.port);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 200);

    for t in &config.topics {
        client.subscribe(t.0, QoS::AtMostOnce).await.unwrap();
    }

    while let Ok(notification) = event_loop.poll().await {
        if let Event::Incoming(Incoming::Publish(packet)) = notification {
            info!("received mqtt packet {:?}", packet);

            let topic = String::from_utf8(packet.topic.to_vec()).unwrap();
            info!("topic: {}", topic);

            let payload = String::from_utf8(packet.payload.to_vec()).unwrap();

            let json: Value = serde_json::from_str(&payload).unwrap();

            match config.topics.get(topic.as_str()) {
                None => {}
                Some(values) => {
                    for v in values {
                        match json.as_object().unwrap().get(v) {
                            None => {}
                            Some(value) => {
                                let key = format!("{}:{}", topic, v);
                                let ts = Utc::now().timestamp_millis();
                                let num_value = value.as_f64().unwrap();
                                info!("New value: {} {} {}", key, ts, num_value);

                                match add_value(&pool, key.as_str(), num_value).await {
                                    Ok(_) => {}
                                    Err(err) => {
                                        error!("Failed to add value: {}", err);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

async fn add_value(pool: &PgPool, topic: &str, value: f64) -> Result<()> {
    let rec = query!(
        r#"
        INSERT INTO values (time, topic, value)
        VALUES (
            NOW(),
            $1,
            $2
        )
        "#,
        topic,
        value
    ).execute(pool).await?;

    Ok(())
}
