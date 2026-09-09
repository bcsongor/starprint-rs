//! Reading a request body, and refusing one that is the wrong type or
//! too big. Nothing here knows what a printer is.

use axum::body::Bytes;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{HeaderMap, StatusCode, header};
use serde::de::DeserializeOwned;

use crate::problem::Problem;

/// A job description is text; anything larger is a client gone wrong.
pub const JSON_LIMIT: usize = 1 << 20;
/// Enough for a phone photo or a long raster job.
pub const BINARY_LIMIT: usize = 16 << 20;

/// The essence of `Content-Type`, lowercased, without its parameters.
/// A handler matches on this to pick which reader to call.
pub fn media_type(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    let essence = value.split_once(';').map_or(value, |(essence, _)| essence);
    Some(essence.trim().to_ascii_lowercase())
}

/// The `415` for a body this endpoint has no reader for.
pub fn unsupported(given: Option<&str>, wanted: &str) -> Problem {
    Problem::unsupported_media_type(match given {
        Some(given) => format!("`{given}` is not a body this endpoint takes; send {wanted}."),
        None => format!("No `Content-Type`; send {wanted}."),
    })
}

/// The whole body, or a `413` if it runs past `limit`.
/// `to_bytes` also reports dropped connections as opaque errors; these
/// receive the same `413` response.
pub async fn collect(request: Request, limit: usize) -> Result<Bytes, Problem> {
    axum::body::to_bytes(request.into_body(), limit)
        .await
        .map_err(|e| Problem::too_large(format!("The body is larger than {limit} bytes: {e}.")))
}

/// A JSON body, read as a `T`. Anything else is a `415`, a body past
/// [`JSON_LIMIT`] a `413`, and JSON that is not a `T` a `400`.
pub async fn json<T: DeserializeOwned>(request: Request) -> Result<T, Problem> {
    match media_type(request.headers()).as_deref() {
        Some("application/json") => {}
        other => return Err(unsupported(other, "application/json")),
    }
    let bytes = collect(request, JSON_LIMIT).await?;
    parse_json(&bytes)
}

/// `bytes` as a `T`, or the `400` for JSON that is not one. The form's
/// `job` part goes through here too, so the wording is the same.
pub fn parse_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Problem> {
    serde_json::from_slice(bytes)
        .map_err(|e| Problem::bad_request(format!("The body could not be read as JSON: {e}.")))
}

/// The `job` part, which is required, and the `image` part, which is
/// not. What they mean is the caller's business.
///
/// Streams the form rather than collecting it, so the size limit comes
/// from the `DefaultBodyLimit` on the route.
pub async fn form(request: Request) -> Result<(Bytes, Option<Bytes>), Problem> {
    let mut multipart = Multipart::from_request(request, &())
        .await
        .map_err(|e| Problem::new(e.status(), format!("The form could not be read: {e}.")))?;
    let (mut job, mut image) = (None, None);
    while let Some(field) = multipart.next_field().await.map_err(part_problem)? {
        let name = field.name().unwrap_or_default().to_owned();
        let slot = match name.as_str() {
            "job" => &mut job,
            "image" => &mut image,
            other => {
                return Err(Problem::bad_request(format!(
                    "`{other}` is not a part this endpoint takes; send `job` and `image`."
                )));
            }
        };
        *slot = Some(field.bytes().await.map_err(part_problem)?);
    }
    let job = job.ok_or_else(|| Problem::bad_request("The form has no `job` part."))?;
    Ok((job, image))
}

fn part_problem(error: axum::extract::multipart::MultipartError) -> Problem {
    let detail = format!("The form could not be read: {error}.");
    match error.status() {
        StatusCode::PAYLOAD_TOO_LARGE => Problem::too_large(detail),
        _ => Problem::bad_request(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn headers(content_type: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
        headers
    }

    #[test]
    fn a_media_type_drops_its_parameters_and_its_case() {
        assert_eq!(
            media_type(&headers("Application/JSON; charset=utf-8")).as_deref(),
            Some("application/json")
        );
        assert_eq!(
            media_type(&headers("multipart/form-data; boundary=xyz")).as_deref(),
            Some("multipart/form-data")
        );
        assert_eq!(media_type(&HeaderMap::new()), None);
    }

    #[tokio::test]
    async fn a_body_past_its_limit_is_a_413_and_a_short_one_is_read() {
        let request = |bytes: usize| Request::new(Body::from(vec![b'x'; bytes]));
        assert_eq!(collect(request(8), 8).await.unwrap().len(), 8);

        let problem = collect(request(9), 8).await.unwrap_err();
        let json = serde_json::to_value(&problem).unwrap();
        assert_eq!(json["status"], 413);
        assert!(
            json["detail"]
                .as_str()
                .unwrap()
                .starts_with("The body is larger than 8 bytes"),
            "{json:?}"
        );
    }

    #[tokio::test]
    async fn a_json_body_is_read_as_its_type_or_refused() {
        let request = |content_type: &str, body: &'static str| {
            Request::builder()
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(body))
                .unwrap()
        };
        let read: Vec<u8> = json(request("application/json; charset=utf-8", "[1, 2]"))
            .await
            .unwrap();
        assert_eq!(read, [1, 2]);

        let status = |problem: Problem| serde_json::to_value(problem).unwrap()["status"].clone();
        let wrong_type = json::<Vec<u8>>(request("application/json", "{}"))
            .await
            .unwrap_err();
        assert_eq!(status(wrong_type), 400);
        let not_json = json::<Vec<u8>>(request("text/plain", "[1]"))
            .await
            .unwrap_err();
        assert_eq!(status(not_json), 415);
    }

    #[test]
    fn an_unsupported_type_names_what_was_wanted() {
        let json =
            serde_json::to_value(unsupported(Some("text/plain"), "application/json")).unwrap();
        assert_eq!(json["status"], 415);
        assert_eq!(
            json["detail"],
            "`text/plain` is not a body this endpoint takes; send application/json."
        );

        let json = serde_json::to_value(unsupported(None, "application/json")).unwrap();
        assert_eq!(json["detail"], "No `Content-Type`; send application/json.");
    }
}
