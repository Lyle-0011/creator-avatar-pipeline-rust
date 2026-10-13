use std::{env, process};

use creator_avatar_pipeline::{AvatarPipeline, InfraiClient};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("avatar pipeline: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let user_id = args
        .next()
        .ok_or("usage: avatar-pipeline <user-id> <image-path>")?;
    let image_path = args
        .next()
        .ok_or("usage: avatar-pipeline <user-id> <image-path>")?;
    if args.next().is_some() {
        return Err("usage: avatar-pipeline <user-id> <image-path>".into());
    }

    let bytes = tokio::fs::read(&image_path).await?;
    let filename = std::path::Path::new(&image_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("image path must end in a UTF-8 filename")?;

    let pipeline = AvatarPipeline::new(InfraiClient::from_env()?);
    let result = pipeline.run(&user_id, filename, bytes).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "user_id": result.user_id,
            "avatar": result.avatar,
            "subscriber_update": {
                "subject": result.subscriber_update.subject,
                "creator_id": result.subscriber_update.creator_id
            }
        }))?
    );
    Ok(())
}
