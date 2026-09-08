use rs1090::data::airports::{Airport, AIRPORTS};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use futures::stream::{self, Stream, StreamExt};
use rs1090::decode::ICAO;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tower_http::cors::{Any, CorsLayer};

use crate::filters::{FilterKey, Filters};
use crate::sensor::Sensor;
use crate::snapshot::{Snapshot, StateVectors};
use crate::SharedState;

/// A decoded message as broadcast to /stream subscribers. The JSON is
/// serialized once by the decoder and shared by every client.
pub struct StreamEvent {
    pub key: FilterKey,
    pub json: String,
}

/// Information required to ask for a trajectory
#[derive(Deserialize)]
pub struct TrackQuery {
    icao24: String,
    since: Option<f64>,
}

/// Information required to search for airports
#[derive(Deserialize)]
pub struct AirportQuery {
    q: String,
}

/// Optional filters for the live stream, as comma-separated lists
#[derive(Deserialize)]
pub struct StreamQuery {
    icao24: Option<String>,
    df: Option<String>,
}

/// An API error serializable to JSON
#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

/// Returns all the ICAO 24-bit addresses of aircraft seen by jet1090
async fn icao24(State(shared): State<Arc<SharedState>>) -> impl IntoResponse {
    let state_vectors = shared.state_vectors.read().await;
    let keys: Vec<_> =
        state_vectors.keys().map(|key| key.to_string()).collect();
    Json(keys)
}

/// Returns all state vectors without any history information
async fn all(State(shared): State<Arc<SharedState>>) -> impl IntoResponse {
    let state_vectors = shared.state_vectors.read().await;
    let snapshots: Vec<&Snapshot> =
        state_vectors.values().map(|sv| &sv.cur).collect();
    Json(
        serde_json::to_value(snapshots)
            .unwrap_or(serde_json::Value::Array(vec![])),
    )
}

/// Returns the trajectory of a given aircraft matching the REST query
async fn track(
    State(shared): State<Arc<SharedState>>,
    Query(q): Query<TrackQuery>,
) -> impl IntoResponse {
    let state_vectors = shared.state_vectors.read().await;
    let res = state_vectors.get(&q.icao24).map(|sv| &sv.hist);
    match q.since {
        Some(since) => Json(serde_json::json!(res.map(|r| r
            .iter()
            .filter(|m| m.timestamp > since)
            .collect::<Vec<_>>()))),
        None => Json(serde_json::json!(res)),
    }
}

/// Downlink formats the decoder can produce, so a typo like `df=71` gets a
/// 400 instead of a stream that stays silent forever
const DOWNLINK_FORMATS: [u16; 11] = [0, 4, 5, 11, 16, 17, 18, 19, 20, 21, 24];

/// Parses a comma-separated query parameter. An absent or empty list
/// becomes None, which the filters treat as "everything".
fn parse_list<T: FromStr>(
    value: Option<&str>,
    name: &str,
    valid: impl Fn(&T) -> bool,
) -> Result<Option<Vec<T>>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let list = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<T>()
                .ok()
                .filter(&valid)
                .ok_or_else(|| format!("invalid {name}: {s}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(if list.is_empty() { None } else { Some(list) })
}

fn stream_filters(q: &StreamQuery) -> Result<Filters, String> {
    Ok(Filters {
        aircraft_filter: parse_list(
            q.icao24.as_deref(),
            "icao24",
            |icao: &ICAO| icao.0 <= 0xffffff,
        )?,
        df_filter: parse_list(q.df.as_deref(), "df", |df: &u16| {
            DOWNLINK_FORMATS.contains(df)
        })?,
    })
}

/// Streams decoded messages as server-sent events, one message per event,
/// in the same JSON format as the `.jsonl` output
async fn stream_messages(
    State(shared): State<Arc<SharedState>>,
    Query(q): Query<StreamQuery>,
) -> Result<
    Sse<impl Stream<Item = Result<Event, Infallible>>>,
    (StatusCode, Json<ErrorMessage>),
> {
    let filters = stream_filters(&q).map_err(|message| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorMessage {
                code: StatusCode::BAD_REQUEST.as_u16(),
                message,
            }),
        )
    })?;

    let rx = shared.stream_tx.subscribe();
    let events = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => return Some((event, rx)),
                Err(RecvError::Lagged(skipped)) => {
                    tracing::debug!(
                        skipped,
                        "slow /stream client skipped messages"
                    );
                    // A plain retry would resume from the oldest buffered
                    // message and leave the client permanently behind
                    rx = rx.resubscribe();
                }
                Err(RecvError::Closed) => return None,
            }
        }
    })
    .filter(move |event| futures::future::ready(filters.matches(&event.key)))
    .map(|event| Ok(Event::default().data(&event.json)));

    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}

/// Returns decoding information about all sensors
async fn sensors(State(shared): State<Arc<SharedState>>) -> impl IntoResponse {
    let state_vectors = shared.state_vectors.read().await;
    Json(sensor_stats(&shared.sensors, &state_vectors))
}

/// Fill in the aircraft count and last timestamp of each sensor.
///
/// A sensor counts an aircraft when it received that aircraft's latest
/// message. Counts across sensors can therefore add up to more than the
/// number of aircraft, since several sensors may receive the same message.
fn sensor_stats(
    sensors: &BTreeMap<u64, Sensor>,
    state_vectors: &BTreeMap<String, StateVectors>,
) -> BTreeMap<u64, Sensor> {
    let mut sensors = sensors.clone();
    let mut seen = HashSet::new();
    for sv in state_vectors.values() {
        // Metadata can list the same sensor several times for one aircraft,
        // which must still only count once
        seen.clear();
        for meta in &sv.cur.metadata {
            if let Some(sensor) = sensors.get_mut(&meta.serial) {
                if seen.insert(meta.serial) {
                    sensor.aircraft_count += 1;
                }
                sensor.last_timestamp =
                    sensor.last_timestamp.max(meta.system_timestamp as u64);
            }
        }
    }
    sensors
}

/// Returns a list of potential airports matching the query string
async fn airports(Query(query): Query<AirportQuery>) -> impl IntoResponse {
    let lowercase = query.q.to_lowercase();
    let res: Vec<&Airport> = AIRPORTS
        .iter()
        .filter(|a| {
            a.name.to_lowercase().contains(&lowercase)
                || a.icao.to_lowercase().contains(&lowercase)
                || a.iata.to_lowercase().contains(&lowercase)
        })
        .collect();
    Json(res)
}

/// Home page with API documentation
async fn home() -> Html<&'static str> {
    Html(
        "Welcome to the jet1090 REST API!<br>\
        Try one of the following routes:<br>\
        <ul>\
        <li><a href=\"/all\">/all</a>: returns all current state vectors</li>\
        <li><a href=\"/icao24\">/icao24</a>: returns all ICAO 24-bit addresses seen</li>\
        <li>/track?icao24={icao24}&amp;since={timestamp}: returns the trajectory of a given aircraft since the given timestamp (optional)</li>\
        <li><a href=\"/sensors\">/sensors</a>: returns information about all sensors</li>\
        <li>/airports?q={string}: returns a list of potential airports matching the query string</li>\
        <li><a href=\"/stream\">/stream</a>: live feed of decoded messages as server-sent events, filter with icao24={a,b} and df={17,18}</li>\
        </ul>",
    )
}

/// Fallback handler for unknown routes
async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorMessage {
            code: StatusCode::NOT_FOUND.as_u16(),
            message: "Route not found, try one of /, /all, /icao24, /track?icao24={icao24}, /sensors, /airports?q={string} or /stream".into(),
        }),
    )
        .into_response()
}

pub async fn serve_web_api(shared: Arc<SharedState>, port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    let app = Router::new()
        .route("/", get(home))
        .route("/icao24", get(icao24))
        .route("/all", get(all))
        .route("/track", get(track))
        .route("/sensors", get(sensors))
        .route("/airports", get(airports))
        .route("/stream", get(stream_messages))
        .fallback(not_found)
        .with_state(shared)
        .layer(cors);

    let listener =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port))
            .await
            .expect("failed to bind port");
    axum::serve(listener, app)
        .await
        .expect("web API server error");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(icao24: Option<&str>, df: Option<&str>) -> StreamQuery {
        StreamQuery {
            icao24: icao24.map(str::to_string),
            df: df.map(str::to_string),
        }
    }

    #[test]
    fn stream_filters_absent_means_everything() {
        let filters = stream_filters(&query(None, None)).unwrap();
        assert!(filters.df_filter.is_none());
        assert!(filters.aircraft_filter.is_none());
    }

    #[test]
    fn stream_filters_parse_lists() {
        let filters =
            stream_filters(&query(Some("3c6444, 4CA7B6"), Some("17,018")))
                .unwrap();
        assert_eq!(
            filters.aircraft_filter,
            Some(vec![ICAO(0x3c6444), ICAO(0x4ca7b6)])
        );
        assert_eq!(filters.df_filter, Some(vec![17, 18]));
    }

    #[test]
    fn stream_filters_treat_empty_lists_as_absent() {
        let filters = stream_filters(&query(Some(""), Some(",,"))).unwrap();
        assert!(filters.aircraft_filter.is_none());
        assert!(filters.df_filter.is_none());
    }

    #[test]
    fn stream_filters_reject_garbage() {
        let error =
            |icao24, df| stream_filters(&query(icao24, df)).unwrap_err();
        assert_eq!(error(Some("zzz"), None), "invalid icao24: zzz");
        assert_eq!(error(Some("1ffffff"), None), "invalid icao24: 1ffffff");
        assert_eq!(error(None, Some("17,abc")), "invalid df: abc");
        assert_eq!(error(None, Some("71")), "invalid df: 71");
    }
}
