use crate::types::{State, UuidOrString};
use axum::{
    Router,
    extract::{Path, State as AxumState},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use image::{DynamicImage::ImageRgb8, DynamicImage::ImageRgba8, imageops};
use uuid::Uuid;

mod bytes;
mod cache;
mod img;
mod mojang;
mod types;

async fn health() -> &'static str {
    "OK"
}

async fn clear_cache(AxumState(data): AxumState<State>, Path(password): Path<String>) -> Response {
    if password == data.clear_cache_password {
        let mut con = data.connection.clone();
        let _: () = redis::cmd("FLUSHALL").query_async(&mut con).await.unwrap();
        return "Cache cleared!".into_response();
    }
    "Wrong password!".into_response()
}

async fn get_avatar(
    AxumState(data): AxumState<State>,
    Path((uuid, size, helm)): Path<(String, u32, bool)>,
) -> Response {
    let uuid: UuidOrString = uuid.parse().unwrap();
    let identifier = cache::create_id(uuid.clone(), helm);

    if size > 512 || size < 8 || size % 8 != 0 {
        return (
            StatusCode::BAD_REQUEST,
            "Size must be between 8 and 512, and divisible by 8.",
        )
            .into_response();
    }

    let mut con = data.connection.clone();
    // STEP: Check if the avatar is cached, if so, load it and return it
    let key: Result<Vec<u8>, _> = cache::get(&identifier, &mut con).await;
    match key {
        Ok(mut buffer) => {
            buffer = bytes::repair(buffer);
            let avatar = match image::load_from_memory(&buffer) {
                Ok(avatar) => avatar.to_rgb8(),
                Err(_) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Error loading avatar from cache!",
                    )
                        .into_response();
                }
            };

            if size != 8 {
                let avatar = img::resize(&avatar, size);
                buffer = img::encode_png(ImageRgb8(avatar));
            }

            return image_response(buffer);
        }
        Err(_) => {}
    }

    let uuid = match uuid {
        UuidOrString::Uuid(uuid) => uuid,
        UuidOrString::String(username) => match mojang::get_uuid(username).await {
            uuid if uuid != Uuid::nil() => uuid,
            _ => {
                return (StatusCode::NOT_FOUND, "User not found!").into_response();
            }
        },
    };

    let skin = match mojang::get_skin(uuid).await {
        Ok(skin) => skin,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Skin not found!").into_response();
        }
    };

    let mut avatar = img::crop(skin.clone(), 8, 8, 8, 8);

    if helm == true {
        let helm = img::crop(skin.to_vec(), 40, 8, 8, 8);
        imageops::overlay(&mut avatar, &helm, 0, 0);
    }

    let buffer: Vec<u8> = img::encode_png(ImageRgb8(avatar.clone()));

    // STEP: Creating the cache entry
    let mut avatar_buffer = buffer.to_vec();
    avatar_buffer = bytes::strip(avatar_buffer);
    cache::set(identifier.clone(), avatar_buffer, &mut con).await;

    if avatar.width() != size {
        avatar = img::resize(&avatar, size);
    } else {
        return image_response(buffer);
    }

    image_response(img::encode_png(ImageRgb8(avatar)))
}

async fn get_skin(Path((uuid, size)): Path<(String, u32)>) -> Response {
    let uuid: UuidOrString = uuid.parse().unwrap();
    if size > 512 || size < 64 || size % 64 != 0 {
        return (
            StatusCode::BAD_REQUEST,
            "Size must be between 64 and 512, and divisible by 64.",
        )
            .into_response();
    }

    let uuid = match uuid {
        UuidOrString::Uuid(uuid) => uuid,
        UuidOrString::String(username) => match mojang::get_uuid(username).await {
            uuid if uuid != Uuid::nil() => uuid,
            _ => {
                return (StatusCode::NOT_FOUND, "User not found!").into_response();
            }
        },
    };

    let skin = match mojang::get_skin(uuid).await {
        Ok(skin) => skin,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                [
                    (header::SERVER, "Ziria"),
                    (header::CACHE_CONTROL, "max-age=1200"),
                ],
                "Skin not found!",
            )
                .into_response();
        }
    };

    let mut skin = image::load_from_memory(&skin).unwrap().to_rgba8();

    if size != 64 {
        skin = img::resize(&skin, size);
    }

    let buffer = img::encode_png(ImageRgba8(skin));
    image_response(buffer)
}

async fn get_skin_64(Path(uuid): Path<String>) -> Response {
    let uuid: UuidOrString = uuid.parse().unwrap();
    let uuid = match uuid {
        UuidOrString::Uuid(uuid) => uuid,
        UuidOrString::String(username) => match mojang::get_uuid(username).await {
            uuid if uuid != Uuid::nil() => uuid,
            _ => {
                return (StatusCode::NOT_FOUND, "User not found!").into_response();
            }
        },
    };

    let skin = match mojang::get_skin(uuid).await {
        Ok(skin) => skin,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Skin not found!").into_response();
        }
    };

    let skin = image::load_from_memory(&skin).unwrap().to_rgba8();

    let buffer = img::encode_png(ImageRgba8(skin));
    image_response(buffer)
}

fn image_response(body: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "max-age=1200"),
            (header::SERVER, "Ziria"),
        ],
        body,
    )
        .into_response()
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let connection_string = format!(
        "{}://{}:{}@{}:{}/",
        std::env::var("REDIS_SCHEME").expect("REDIS_SCHEME is required"),
        std::env::var("REDIS_USERNAME").expect("REDIS_USERNAME is required"),
        std::env::var("REDIS_PASSWORD").expect("REDIS_PASSWORD is required"),
        std::env::var("REDIS_HOST").expect("REDIS_HOST is required"),
        std::env::var("REDIS_PORT").expect("REDIS_PORT is required"),
    );

    let client = redis::Client::open(connection_string).unwrap();
    let con = client.get_multiplexed_async_connection().await.unwrap();
    let state = State {
        connection: con,
        clear_cache_password: std::env::var("CLEAR_CACHE_PASSWORD")
            .expect("CLEAR_CACHE_PASSWORD is required"),
    };

    let app = Router::new()
        .route("/", get(health))
        .route("/clear_cache/{password}", get(clear_cache))
        .route("/avatar/{uuid}/{size}/{helm}", get(get_avatar))
        .route("/skin/{uuid}/{size}", get(get_skin))
        .route("/skin/{uuid}", get(get_skin_64))
        .with_state(state);

    let port = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|args| args[0] == "--port")
        .map(|args| args[1].clone())
        .or_else(|| std::env::var("PORT").ok())
        .unwrap_or_else(|| "8000".to_string());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind listener");

    axum::serve(listener, app).await.expect("server failed");
}
