use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    Json,
    response::IntoResponse,
    Router, routing::get,
};
use serde::Deserialize;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let global_price = Arc::new(RwLock::new(None));
    let app = app(global_price);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn app(state: GlobalPrice) -> Router {
    Router::new()
        .route("/price", get(get_price).patch(set_price).delete(set_null_price))
        .with_state(state)
}

async fn get_price(
    State(global_price): State<GlobalPrice>,
) -> Result<impl IntoResponse, StatusCode> {
    let global_price = global_price.read().await;
    if let Some(price) = *global_price {
        Ok(price.to_string())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, Deserialize)]
struct PriceDto {
    price: u64,
}

async fn set_price(
    State(global_price): State<GlobalPrice>,
    Json(input): Json<PriceDto>,
) -> Result<impl IntoResponse, StatusCode> {
    let price = input.price;
    let mut global_price = global_price.write().await;
    *global_price = Some(price);

    Ok(StatusCode::OK)
}

async fn set_null_price(
    State(global_price): State<GlobalPrice>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut global_price = global_price.write().await;
    *global_price = None;

    Ok(StatusCode::OK)
}

type GlobalPrice = Arc<RwLock<Option<u64>>>;

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use axum::body::Bytes;
    use axum::response::Response;
    use axum::routing::RouterIntoService;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::{Service, ServiceExt};

    use super::*;

    #[tokio::test]
    async fn get_price_test() {
        let state = Arc::new(RwLock::new(Some(100)));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::GET,
            "/price",
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(collect_body(response).await, "100");
    }

    #[tokio::test]
    async fn get_not_found_price_test() {
        let state = Arc::new(RwLock::new(None));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::GET,
            "/price",
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(collect_body(response).await, "");
    }

    #[tokio::test]
    async fn patch_price_test() {
        let state = Arc::new(RwLock::new(None));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::PATCH,
            "/price",
            Some(&json!({"price": 355}))
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::OK);

        let request = build_request(
            http::Method::GET,
            "/price",
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(collect_body(response).await, "355");
    }

    #[tokio::test]
    async fn delete_price_test() {
        let state = Arc::new(RwLock::new(Some(5)));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::DELETE,
            "/price",
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::OK);

        let request = build_request(
            http::Method::GET,
            "/price",
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(collect_body(response).await, "");
    }

    fn build_request(method: http::Method, uri: &str, maybe_json: Option<&Value>) -> Request<Body> {
        let body = match maybe_json {
            Some(json) => Body::from(
                serde_json::to_vec(json).unwrap(),
            ),
            None => Body::empty(),
        };

        Request::builder()
            .method(method)
            .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .uri(uri)
            .body(body)
            .unwrap()
    }

    async fn call(request: Request<Body>, app: &mut RouterIntoService<Body>) -> Response<Body> {
        ServiceExt::<Request<Body>>::ready(app)
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap()
    }

    async fn collect_body(response: Response<Body>) -> Bytes {
        response.into_body().collect().await.unwrap().to_bytes()
    }
}
