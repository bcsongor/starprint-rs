//! MCP at `/mcp`. The routes an agent has a use for are tools here,
//! over streamable HTTP on the same port and behind the same token. A
//! tool does what its route does by calling what the route calls, so a
//! job sent either way is the same job in the same queue.
//!
//! An agent has only the tools' descriptions and their arguments'
//! schemas to go by, so those carry what it must know: printing spends
//! paper, a success is a completed write and not a print, and a failure
//! may have printed. The schemas come from the types that read the
//! arguments, whose comments are the descriptions.
//!
//! There is no tool for raw bytes, for changing a profile or for
//! setting up the fax line. A person does those, in the desktop app or
//! over the routes, and a description is a weaker fence than a missing
//! tool.

use std::sync::Arc;

use axum::body::Bytes;
use rmcp::handler::server::tool::{IntoCallToolResult, ToolCallContext};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use starprint_fax::base64_bytes;
use starprint_workflows::{Job, Preview, Speed};

use crate::body;
use crate::faxing::{self, ContactRequest, SendRequest};
use crate::job::JobRequest;
use crate::printers::Printers;
use crate::problem::Problem;
use crate::schedule::{self, ScheduleSpec};

/// A call's JSON, with room for a picture of [`body::BINARY_LIMIT`] as
/// base64, which is a third larger.
const BODY_LIMIT: usize = body::BINARY_LIMIT.div_ceil(3) * 4 + body::JSON_LIMIT;

/// What a client is told once, on connecting. A client may not show
/// this to its model, so each tool's description repeats what that tool
/// needs said.
const INSTRUCTIONS: &str = "\
Prints to Star receipt printers on the local network: task cards, text, note slips, QR codes, \
pictures and test pages. It also prints them on a schedule and faxes them to other people's \
printers.

Printing is physical and cannot be undone. It spends paper, and ribbon on an impact printer. \
Print or fax what the user asked for, once, and ask before doing either unasked. `preview` shows \
a job without printing it.

Every job goes to a printer by name, so call `list_printers` first.

An error is an RFC 9457 problem. Its `status` is what an HTTP client would get, and its `detail` \
says what went wrong in a sentence worth reading back to the user. A print that failed may still \
have printed, so do not retry one.

Printer profiles, the fax line and its relays are the user's to set up, in the Starprint desktop \
app.";

/// The service `app::router` mounts at `/mcp`.
pub fn service(printers: Arc<Printers>) -> StreamableHttpService<Mcp, NeverSessionManager> {
    let config = StreamableHttpServerConfig::default()
        // The server keeps nothing between calls and answers each as
        // plain JSON, so a client survives a restart and no stream
        // stays open to hold a shutdown up.
        .with_legacy_session_mode(false)
        .with_json_response(true)
        // No `Host` check. The server answers at whichever address it
        // was bound to, and the token admits a request, as for the
        // routes.
        .disable_allowed_hosts()
        .with_max_request_body_bytes(BODY_LIMIT);
    StreamableHttpService::new(
        move || {
            Ok(Mcp {
                printers: Arc::clone(&printers),
            })
        },
        Arc::default(),
        config,
    )
}

/// The tools. The service makes one for every call.
pub struct Mcp {
    printers: Arc<Printers>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PrinterArgs {
    /// A printer's `name`, from `list_printers`.
    printer: String,
}

/// A job request as `/jobs` takes it, with the printer a route has in
/// its path and the picture a form has in a part.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobArgs {
    /// A printer's `name`, from `list_printers`.
    printer: String,
    /// `kind` picks the job, and the rest are that job's own settings.
    /// Only its content is required.
    job: Job,
    /// Whether the job ends with a cut. Left out, the profile decides.
    cut: Option<bool>,
    /// Thermal only, and refused on an impact printer. 4 is two-colour
    /// mode, a darker black on plain paper. Left out, the profile
    /// decides, and its setting was tuned on the printer.
    #[schemars(range(min = -3, max = 4))]
    density: Option<i8>,
    /// Thermal only, and refused on an impact printer. Does nothing at
    /// density 4. Left out, the profile decides.
    speed: Option<Speed>,
    /// A picture job's picture, as base64: PNG, JPEG, WebP or BMP. It
    /// travels in the call, so only a small one fits. Post a file to
    /// `/v1/printers/{printer}/jobs` on this server instead, as
    /// `multipart/form-data` under the same token, with `job` and the
    /// overrides as JSON in a `job` part and the file in an `image`
    /// part.
    #[serde(default, with = "base64_bytes::option")]
    #[schemars(with = "Option<String>")]
    image: Option<Vec<u8>>,
}

impl JobArgs {
    fn request(self) -> Result<(JobRequest, Option<Bytes>), Problem> {
        let image = image_for(&self.job, self.image)?;
        let request = JobRequest {
            job: self.job,
            cut: self.cut,
            density: self.density,
            speed: self.speed,
        };
        Ok((request, image))
    }
}

/// A schedule as `list_schedules` gives one: what `create_schedule`
/// takes, with the id a route has in its path.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScheduleArgs {
    /// The schedule's `id`, from `list_schedules`.
    id: String,
    #[serde(flatten)]
    schedule: ScheduleSpec,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScheduleId {
    /// The schedule's `id`, from `list_schedules`.
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FaxArgs {
    /// A number, like `*star1en2su3z68yscvky0n3j3l2qwny4dkq7s`, or the
    /// name someone has in the fax book.
    to: String,
    /// `kind` picks the job, and the rest are that job's own settings.
    /// It prints with the other printer's settings, and may be up to
    /// 4,000 characters as JSON with 100 line breaks.
    job: Job,
    /// A picture job's picture, as base64: PNG, JPEG, WebP or BMP, up
    /// to 50 megapixels and four times as tall as it is wide. It
    /// travels in the call, so only a small one fits. Post a file to
    /// `/v1/fax/send` on this server instead, as `multipart/form-data`
    /// under the same token, with `to` and `job` as JSON in a `job`
    /// part and the file in an `image` part.
    #[serde(default, with = "base64_bytes::option")]
    #[schemars(with = "Option<String>")]
    image: Option<Vec<u8>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ContactArgs {
    /// The number to file, with or without its star.
    number: String,
    /// What to call it, in up to 64 characters. No other number may
    /// have the name.
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NumberArgs {
    /// A number in the fax book, with or without its star.
    number: String,
}

/// The picture as the jobs take it, or the `400` for a picture job
/// without one. The routes' own `400` talks of a form, which a tool
/// does not have.
fn image_for(job: &Job, image: Option<Vec<u8>>) -> Result<Option<Bytes>, Problem> {
    if job.needs_image() && image.is_none() {
        return Err(Problem::bad_request(
            "A picture job needs its picture, as base64 in `image`.",
        ));
    }
    Ok(image.map(Bytes::from))
}

/// What the route answers with, as the tool's text.
fn reply(value: &impl Serialize) -> CallToolResult {
    said(serde_json::to_string(value).expect("a reply serialises"))
}

fn said(text: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
}

/// A preview an agent can look at. The layout goes as JSON, and what is
/// drawn as dots goes as an image beside it, since a model cannot see a
/// `data:` URL in text.
fn shown(preview: &Preview) -> CallToolResult {
    let image = match preview {
        Preview::Note { image, .. } | Preview::Qr { image, .. } | Preview::Picture { image } => {
            Some(image)
        }
        Preview::TaskCard(_) | Preview::Text(_) | Preview::TestPage { .. } => None,
    };
    let mut layout = serde_json::to_value(preview).expect("a preview serialises");
    if let Some(layout) = layout.as_object_mut() {
        layout.remove("image");
    }
    let mut content = vec![ContentBlock::text(layout.to_string())];
    content.extend(image.map(|image| ContentBlock::image(image.base64(), "image/png")));
    CallToolResult::success(content)
}

/// The problem details a route answers with, as the tool's error.
impl IntoCallToolResult for Problem {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        let problem = serde_json::to_string(&self).expect("problem details serialise");
        Ok(CallToolResult::error(vec![ContentBlock::text(problem)]).into())
    }
}

#[tool_router]
impl Mcp {
    /// The printers a job can go to, by `name`, and the server's
    /// version. `notes` is whatever the user wrote about a printer, so
    /// read it when choosing between printers. If there is more than
    /// one and the user did not say which, ask.
    #[tool(annotations(read_only_hint = true))]
    fn list_printers(&self) -> CallToolResult {
        reply(&self.printers.list())
    }

    /// Whether a printer is switched on and answering, without printing
    /// anything. Waits for a job in progress on that printer to finish
    /// first.
    #[tool(annotations(read_only_hint = true))]
    async fn printer_status(
        &self,
        Parameters(args): Parameters<PrinterArgs>,
    ) -> Result<CallToolResult, Problem> {
        Ok(reply(&self.printers.status(&args.printer).await?))
    }

    /// Prints a job. Printing is physical and cannot be undone. It
    /// spends paper, and ribbon on an impact printer. Print what the
    /// user asked for, once, and ask before printing anything they did
    /// not ask for.
    ///
    /// Answers with `bytesSent`, which means the job was written to the
    /// printer and nothing more. The printer cannot say whether it
    /// printed, so report the job as sent, not as printed. A failure
    /// may still have printed some or all of the job, so never send it
    /// again on your own. Tell the user and let them look at the paper.
    #[tool]
    async fn print(
        &self,
        Parameters(args): Parameters<JobArgs>,
    ) -> Result<CallToolResult, Problem> {
        let printer = self.printers.find(&args.printer)?.printer;
        let (request, image) = args.request()?;
        let report = request.print(&printer, image, &self.printers.queue).await?;
        Ok(reply(&report))
    }

    /// Shows a job as it will print, without printing it. Nothing
    /// reaches the printer, so it works while the printer is off. Text
    /// comes back as the `lines` the printer will set and how many
    /// `columns` it has, and anything drawn as dots as an image.
    #[tool(annotations(read_only_hint = true))]
    async fn preview(
        &self,
        Parameters(args): Parameters<JobArgs>,
    ) -> Result<CallToolResult, Problem> {
        let printer = self.printers.find(&args.printer)?.printer;
        let (request, image) = args.request()?;
        Ok(shown(&request.preview(&printer, image).await?))
    }

    /// The jobs that print on a schedule, each with its `id`.
    #[tool(annotations(read_only_hint = true))]
    fn list_schedules(&self) -> CallToolResult {
        reply(&schedule::list(&self.printers.data))
    }

    /// Prints a job on a schedule, on the server's own clock, whether
    /// or not anyone is at the machine. A schedule prints without
    /// asking anyone, every time it fires, so confirm the cron
    /// expression and the job with the user first.
    #[tool]
    async fn create_schedule(
        &self,
        Parameters(spec): Parameters<ScheduleSpec>,
    ) -> Result<CallToolResult, Problem> {
        Ok(reply(&schedule::create(&self.printers.data, spec).await?))
    }

    /// Replaces a schedule whole, so send every field it should keep.
    /// Read what it holds with `list_schedules` first, and change only
    /// what the user asked to.
    #[tool]
    async fn update_schedule(
        &self,
        Parameters(args): Parameters<ScheduleArgs>,
    ) -> Result<CallToolResult, Problem> {
        let replaced = schedule::replace(&self.printers.data, &args.id, args.schedule).await?;
        Ok(reply(&replaced))
    }

    /// Deletes a schedule. Read what exists with `list_schedules`
    /// first, and delete only what the user asked to.
    #[tool]
    fn delete_schedule(
        &self,
        Parameters(args): Parameters<ScheduleId>,
    ) -> Result<CallToolResult, Problem> {
        schedule::remove(&self.printers.data, &args.id)?;
        Ok(said(format!("Schedule {} is deleted.", args.id)))
    }

    /// The fax line: its `number`, the fax book in `contacts`, the
    /// numbers lately faxed or printed from that are not in it in
    /// `recent`, and its `relays`. A `number` of null, or no `relays`,
    /// means faxing is not set up, which the user does in the Starprint
    /// desktop app.
    #[tool(annotations(read_only_hint = true))]
    fn get_fax_line(&self) -> CallToolResult {
        reply(&faxing::view(&self.printers))
    }

    /// Faxes a job to someone else's printer, by their number or by the
    /// name they have in the fax book. A fax prints on someone else's
    /// paper with nobody there to approve it. Confirm who it goes to
    /// and what it says before sending, send it once, and never fax
    /// anyone the user did not name.
    ///
    /// Answers with `name` when the number is in the fax book. Read it
    /// back to the user, since it is how they know the number was
    /// right. The fax prints when the other side next checks its relay,
    /// usually within ten seconds, and waits there while their server
    /// is off.
    ///
    /// A `404` means no one in the fax book has that name, or no relay
    /// has that number. Check it with the user rather than guessing any
    /// part of it. A `400` about a number means a character is
    /// mistyped. A `409` means the line is not set up. A `502` saying
    /// the record's key is not the number's means a relay is answering
    /// for someone else. Tell the user, and do not work around it.
    #[tool]
    async fn send_fax(
        &self,
        Parameters(args): Parameters<FaxArgs>,
    ) -> Result<CallToolResult, Problem> {
        let image = image_for(&args.job, args.image)?;
        let request = SendRequest {
            to: args.to,
            job: args.job,
        };
        Ok(reply(&faxing::send(&self.printers, request, image).await?))
    }

    /// Files a number in the fax book under a name, replacing the name
    /// it had, so faxes can be sent to the name. Offer this after a fax
    /// to a number that was not in the book, and ask what to call it.
    #[tool]
    fn save_fax_contact(
        &self,
        Parameters(args): Parameters<ContactArgs>,
    ) -> Result<CallToolResult, Problem> {
        let number = faxing::parse_number(&args.number)?;
        let request = ContactRequest { name: args.name };
        let contact = faxing::put_contact(&self.printers, number, request)?;
        Ok(reply(&contact))
    }

    /// Takes a number out of the fax book.
    #[tool]
    fn delete_fax_contact(
        &self,
        Parameters(args): Parameters<NumberArgs>,
    ) -> Result<CallToolResult, Problem> {
        let number = faxing::parse_number(&args.number)?;
        faxing::remove_contact(&self.printers, number)?;
        Ok(said(format!("{number} is out of the fax book.")))
    }
}

#[tool_handler]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("starprint", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    /// Runs a tool until it answers or its client goes away. The SDK
    /// runs a tool on a task of its own and only signals that the
    /// client went, where axum drops a route's handler with its
    /// connection. Without this a job still waiting for its printer
    /// would print for nobody, after a shutdown too. A write already
    /// under way finishes under its queue guard either way.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let gone = context.ct.clone();
        let tools = Self::tool_router();
        let call = tools.call(ToolCallContext::new(self, request, context));
        tokio::select! {
            // First, so a job handed its turn as the client went does
            // not start.
            biased;
            () = gone.cancelled() => Err(ErrorData::internal_error("The client went away.", None)),
            answer = call => answer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::router;
    use crate::app::tests::{TOKEN, closed_port, fake_printer, png, printers};
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use serde_json::{Value, json};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::time::timeout;
    use tower::ServiceExt as _;

    /// A call as a client on another machine makes it, so by an address
    /// that is not loopback.
    fn post(body: Value) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri("/mcp")
            .header(header::HOST, "192.168.1.50:9110")
            .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// One call, and its `result`.
    async fn rpc(printers: Arc<Printers>, method: &str, params: Value) -> Value {
        let call = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let response = router(printers).oneshot(post(call)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let mut reply: Value = serde_json::from_slice(&body).unwrap();
        assert!(reply.get("error").is_none(), "{reply}");
        reply["result"].take()
    }

    async fn tool(printers: Arc<Printers>, name: &str, arguments: Value) -> Value {
        let call = json!({ "name": name, "arguments": arguments });
        rpc(printers, "tools/call", call).await
    }

    /// The JSON a tool answered with as its text.
    fn text(result: &Value) -> Value {
        let text = result["content"][0]["text"].as_str().expect("text");
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
    }

    /// The problem details a tool failed with.
    fn problem(result: &Value) -> Value {
        assert_eq!(result["isError"], true, "{result}");
        text(result)
    }

    fn base64(bytes: &[u8]) -> Value {
        base64_bytes::serialize(bytes, serde_json::value::Serializer).unwrap()
    }

    #[tokio::test]
    async fn a_client_learns_what_the_server_is_and_which_tools_it_has() {
        let hello = rpc(
            printers(9100),
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" },
            }),
        )
        .await;
        assert_eq!(hello["serverInfo"]["name"], "starprint");
        assert_eq!(hello["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert!(
            hello["instructions"]
                .as_str()
                .is_some_and(|text| text.contains("cannot be undone")),
            "{hello}"
        );

        let listed = rpc(printers(9100), "tools/list", json!({})).await;
        let tools = listed["tools"].as_array().unwrap();
        let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "create_schedule",
                "delete_fax_contact",
                "delete_schedule",
                "get_fax_line",
                "list_printers",
                "list_schedules",
                "preview",
                "print",
                "printer_status",
                "save_fax_contact",
                "send_fax",
                "update_schedule",
            ],
            "and none for raw bytes, profiles or setting up the fax line"
        );

        let print = tools.iter().find(|t| t["name"] == "print").unwrap();
        assert!(
            print["description"]
                .as_str()
                .is_some_and(|text| text.contains("cannot be undone")),
            "the warning is the tool's own: {print}"
        );
        let kinds: Vec<&str> = print["inputSchema"]["properties"]["job"]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|job| job["properties"]["kind"]["const"].as_str().unwrap())
            .collect();
        assert_eq!(
            kinds,
            ["task-card", "text", "note", "qr", "test-page", "picture"],
            "a job's schema is its type's"
        );
    }

    #[tokio::test]
    async fn mcp_is_behind_the_token() {
        let mut request = post(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }));
        request.headers_mut().remove(header::AUTHORIZATION);
        let response = router(printers(9100)).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn the_printers_and_whether_one_answers_come_without_printing() {
        let printers = printers(closed_port().await);
        let list = text(&tool(Arc::clone(&printers), "list_printers", json!({})).await);
        assert_eq!(list["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(list["printers"][1]["name"], "sp743");

        let status = tool(printers, "printer_status", json!({ "printer": "sp743" })).await;
        assert_eq!(text(&status), json!({ "online": false }));
    }

    #[tokio::test]
    async fn a_tool_prints_as_the_route_does() {
        let (port, received) = fake_printer().await;
        let result = tool(
            printers(port),
            "print",
            json!({
                "printer": "tsp800ii",
                "job": { "kind": "task-card", "text": "Renew passport", "priority": true },
                "cut": false,
            }),
        )
        .await;
        assert_ne!(result["isError"], true, "{result}");

        let bytes = received.await.unwrap();
        assert_eq!(text(&result)["bytesSent"], bytes.len());
        assert!(bytes.windows(14).any(|w| w == b"Renew passport"));
        assert!(
            !bytes.ends_with(&[0x1b, b'd', 3]),
            "the override reached the job"
        );
    }

    /// The SDK runs a tool on a task of its own and only signals when
    /// the client goes, so a job waiting for its printer would otherwise
    /// print for nobody, even after the server had shut down.
    #[tokio::test]
    async fn a_print_whose_client_went_away_never_starts() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let printers = printers(port);
        let turn = printers.queue.lock("127.0.0.1", port).await;
        let print = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "print",
                "arguments": { "printer": "sp743", "job": { "kind": "text", "text": "Hi" } },
            },
        });
        let client = tokio::spawn(router(printers).oneshot(post(print)));
        // Long enough for the job to be built and to wait for its turn.
        tokio::time::sleep(Duration::from_millis(200)).await;
        client.abort();
        let _ = client.await;

        drop(turn);
        assert!(
            timeout(Duration::from_millis(200), listener.accept())
                .await
                .is_err(),
            "nothing was written"
        );
    }

    /// A call has no part to put a picture in, so it travels as base64.
    #[tokio::test]
    async fn a_picture_travels_in_the_call() {
        let (port, received) = fake_printer().await;
        let result = tool(
            printers(port),
            "print",
            json!({
                "printer": "sp743",
                "job": { "kind": "picture", "double": true },
                "image": base64(&png()),
            }),
        )
        .await;
        assert_ne!(result["isError"], true, "{result}");
        assert!(
            received
                .await
                .unwrap()
                .windows(3)
                .any(|w| w == [0x1b, b'^', 1]),
            "a double-density bit image"
        );

        let result = tool(
            printers(9100),
            "print",
            json!({ "printer": "sp743", "job": { "kind": "picture" } }),
        )
        .await;
        let problem = problem(&result);
        assert_eq!(problem["status"], 400);
        assert_eq!(
            problem["detail"], "A picture job needs its picture, as base64 in `image`.",
            "said in the tool's terms, not a form's"
        );
    }

    /// What is drawn as dots comes back as an image the agent can look
    /// at, not as a `data:` URL in the text.
    #[tokio::test]
    async fn a_preview_is_a_layout_and_an_image_beside_it() {
        let printers = printers(closed_port().await);
        let code =
            json!({ "kind": "qr", "data": "https://example.com/r/42", "caption": "Order 42" });
        let result = tool(
            Arc::clone(&printers),
            "preview",
            json!({ "printer": "tsp800ii", "job": code }),
        )
        .await;
        let layout = text(&result);
        assert_eq!(layout["kind"], "qr");
        assert_eq!(layout["caption"][0], "Order 42");
        assert!(layout.get("image").is_none(), "{layout}");
        let image = &result["content"][1];
        assert_eq!(image["type"], "image");
        assert_eq!(image["mimeType"], "image/png");
        assert!(
            image["data"]
                .as_str()
                .is_some_and(|data| data.starts_with("iVBOR")),
            "a PNG: {image}"
        );

        let card = json!({ "kind": "task-card", "text": "Standup" });
        let result = tool(
            printers,
            "preview",
            json!({ "printer": "tsp800ii", "job": card }),
        )
        .await;
        assert_eq!(text(&result)["lines"][0], "Standup");
        assert_eq!(result["content"].as_array().unwrap().len(), 1);
    }

    /// A tool fails with the problem details its route answers with.
    #[tokio::test]
    async fn a_failure_is_the_routes_problem_as_the_tools_error() {
        let job = json!({ "kind": "text", "text": "Hi" });
        let result = tool(
            printers(9100),
            "print",
            json!({ "printer": "nope", "job": job }),
        )
        .await;
        let unknown = problem(&result);
        assert_eq!(unknown["status"], 404);
        assert_eq!(unknown["detail"], "No printer named `nope`.");

        let result = tool(
            printers(closed_port().await),
            "print",
            json!({ "printer": "sp743", "job": job }),
        )
        .await;
        let unreachable = problem(&result);
        assert_eq!(unreachable["status"], 502);
        assert!(
            unreachable["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("none, some or all")),
            "the agent is told not to assume nothing printed: {unreachable}"
        );

        // The address is the profile's alone here too. Arguments that
        // will not read are the tool's error as well, in the SDK's own
        // words and not as problem details.
        let result = tool(
            printers(9100),
            "print",
            json!({ "printer": "sp743", "job": job, "host": "10.0.0.1" }),
        )
        .await;
        assert_eq!(result["isError"], true, "{result}");
    }

    #[tokio::test]
    async fn a_schedule_is_made_changed_and_deleted_through_tools() {
        let printers = printers(9100);
        let schedule = json!({
            "printer": "sp743",
            "cron": "0 9 * * 1-5",
            "due": "run-day",
            "job": { "kind": "task-card", "text": "Standup" },
        });
        let created = text(&tool(Arc::clone(&printers), "create_schedule", schedule).await);
        let id = created["id"].as_str().unwrap().to_owned();

        // What `list_schedules` gives goes back as it came, changed.
        let listed = text(&tool(Arc::clone(&printers), "list_schedules", json!({})).await);
        let mut changed = listed[0].clone();
        assert_eq!(changed["id"], id);
        changed["enabled"] = json!(false);
        let result = tool(Arc::clone(&printers), "update_schedule", changed.clone()).await;
        assert_eq!(text(&result)["enabled"], false);
        assert!(!printers.data.schedules()[0].1.enabled);

        changed["enbled"] = json!(true);
        let result = tool(Arc::clone(&printers), "update_schedule", changed).await;
        assert_eq!(
            result["isError"], true,
            "a misspelt field is refused: {result}"
        );

        let gone = json!({ "id": id });
        let result = tool(Arc::clone(&printers), "delete_schedule", gone.clone()).await;
        assert_ne!(result["isError"], true, "{result}");
        assert!(printers.data.schedules().is_empty());
        let result = tool(printers, "delete_schedule", gone).await;
        assert_eq!(problem(&result)["status"], 404);
    }

    /// The tools reach the fax line; `faxing` tests the rest.
    #[tokio::test]
    async fn the_fax_book_is_kept_through_tools_and_a_fax_needs_the_line() {
        let printers = printers(closed_port().await);
        let line = text(&tool(Arc::clone(&printers), "get_fax_line", json!({})).await);
        assert_eq!(line["number"], Value::Null);

        let anna = "*star15089lwj8gn70m8gepwymguzl5qkangla";
        let contact = json!({ "number": anna, "name": "Anna" });
        let result = tool(Arc::clone(&printers), "save_fax_contact", contact.clone()).await;
        assert_eq!(text(&result), contact);

        let fax = json!({ "to": "Anna", "job": { "kind": "text", "text": "Hi" } });
        let result = tool(Arc::clone(&printers), "send_fax", fax).await;
        assert_eq!(problem(&result)["status"], 409);

        let number = json!({ "number": anna });
        let result = tool(Arc::clone(&printers), "delete_fax_contact", number).await;
        assert_ne!(result["isError"], true, "{result}");
        let typo = json!({ "number": "star1typo" });
        let result = tool(printers, "delete_fax_contact", typo).await;
        assert_eq!(problem(&result)["status"], 400);
    }
}
