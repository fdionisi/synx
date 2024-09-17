use axum::{
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    database::Database,
    message::{CreateMessage, Message, UpdateMessage},
    thread::Thread,
};

async fn update_message(
    State(db): State<Database>,
    Path((thread_id, message_id)): Path<(Uuid, Uuid)>,
    Json(update_message): Json<UpdateMessage>,
) -> Response {
    match db
        .update_message(thread_id, message_id, update_message)
        .await
    {
        Ok(message) => (StatusCode::OK, Json(message)).into_response(),
        Err(e) => match e.to_string().as_str() {
            "thread not found" | "message not found" => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "internal server error" })),
            )
                .into_response(),
        },
    }
}

async fn delete_message(
    State(db): State<Database>,
    Path((thread_id, message_id)): Path<(Uuid, Uuid)>,
) -> StatusCode {
    match db.delete_message(thread_id, message_id).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(e) => match e.to_string().as_str() {
            "thread not found" | "message not found" => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        },
    }
}

async fn delete_thread(State(db): State<Database>, Path(thread_id): Path<Uuid>) -> StatusCode {
    match db.delete_thread(thread_id).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::NOT_FOUND,
    }
}

#[derive(Clone)]
pub struct AppState {
    db: Database,
}

impl AppState {
    pub fn router() -> Router {
        let state = Self {
            db: Database::new(),
        };

        Router::new()
            .route("/threads", post(create_thread))
            .route("/threads", get(list_threads))
            .route("/threads/:id", get(get_thread))
            .route("/threads/:id", delete(delete_thread))
            .route("/threads/:id/messages", post(create_message))
            .route("/threads/:id/messages", get(get_messages))
            .route(
                "/threads/:thread_id/messages/:message_id",
                put(update_message),
            )
            .route(
                "/threads/:thread_id/messages/:message_id",
                delete(delete_message),
            )
            .with_state(state)
            .layer(TraceLayer::new_for_http())
    }
}

async fn create_thread(State(db): State<Database>) -> (StatusCode, Json<serde_json::Value>) {
    let thread = db.create_thread().await;
    let thread_id = thread.id();

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": thread_id })),
    )
}

async fn list_threads(State(db): State<Database>) -> Json<Vec<Thread>> {
    let threads = db.list_threads().await;
    Json(threads)
}

async fn get_thread(
    State(db): State<Database>,
    Path(thread_id): Path<Uuid>,
) -> Result<Json<Thread>, StatusCode> {
    match db.get_thread(thread_id).await {
        Ok(thread) => Ok(Json(thread)),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_messages(
    State(db): State<Database>,
    Path(thread_id): Path<Uuid>,
) -> Result<Json<Vec<Message>>, StatusCode> {
    match db.get_thread_messages(thread_id).await {
        Ok(messages) => Ok(Json(messages)),
        Err(e) => match e.to_string().as_str() {
            "thread not found" => Err(StatusCode::NOT_FOUND),
            _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
        },
    }
}

async fn create_message(
    State(db): State<Database>,
    Path(thread_id): Path<Uuid>,
    Json(create_message): Json<CreateMessage>,
) -> Response {
    match db.create_message(thread_id, create_message).await {
        Ok(message) => (StatusCode::CREATED, Json(message)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "thread not found" })),
        )
            .into_response(),
    }
}

impl FromRef<AppState> for Database {
    fn from_ref(app_state: &AppState) -> Database {
        app_state.db.clone()
    }
}
