//! RFC 9457 problem details, the only error shape the API returns.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// `application/problem+json`, per RFC 9457 §3.
#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    /// Left as `about:blank`, which RFC 9457 §4.2.1 reserves for
    /// problems that carry no meaning beyond their status code.
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    #[serde(serialize_with = "as_u16")]
    status: StatusCode,
    detail: String,
}

fn as_u16<S: serde::Serializer>(status: &StatusCode, out: S) -> Result<S::Ok, S::Error> {
    out.serialize_u16(status.as_u16())
}

impl Problem {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            kind: "about:blank",
            title: status.canonical_reason().unwrap_or("Error"),
            status,
            detail: detail.into(),
        }
    }

    /// Malformed JSON, invalid options, options the printer kind does
    /// not accept, or a job that cannot be built.
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, detail)
    }

    pub fn too_large(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, detail)
    }

    pub fn unsupported_media_type(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, detail)
    }

    /// DNS, connection, timeout or write failure while contacting the
    /// printer. The printer may have received none, some or all of the
    /// bytes, so a client must not retry on its own.
    pub fn bad_gateway(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, detail)
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let body = serde_json::to_vec(&self).expect("problem details serialise");
        (
            self.status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            body,
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_carries_its_status_in_the_body_and_the_response() {
        let problem = Problem::not_found("Unknown printer `nope`.");
        let json = serde_json::to_value(&problem).unwrap();
        assert_eq!(json["type"], "about:blank");
        assert_eq!(json["title"], "Not Found");
        assert_eq!(json["status"], 404);
        assert_eq!(json["detail"], "Unknown printer `nope`.");

        let response = problem.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
    }
}
