use std::{collections::BTreeMap, sync::Arc, time::Instant};

use http::{HeaderValue, Method, StatusCode, header::ALLOW};
use matchit::Router as PathMatcher;
use server_native::{
    Handler, HandlerFuture, NativeResponse, NativeResponseFuture, ServerRequest, ServerResponse,
};
use vut_native_sdk::{CallbackResult, List, Nominal, Runtime, Str, Value};

use crate::{
    context::{self, ContextBuildError},
    identity::{Controller, DispatchPipelineType, RouteEntryType},
    request::{self, Endpoint, PathRoute},
    response, terminal,
};

pub(crate) struct RouteTable {
    matcher: PathMatcher<Arc<PathRoute>>,
}

impl RouteTable {
    pub(crate) fn from_entries(
        entries: &Value<List<Nominal<RouteEntryType>>>,
    ) -> Result<Self, RouteLoadError> {
        let entry_count = entries
            .len()
            .map_err(|error| RouteLoadError::Abi(format!("route list length: {error}")))?;
        let mut grouped: BTreeMap<String, BTreeMap<Method, Arc<Endpoint>>> = BTreeMap::new();

        for route_index in 0..entry_count {
            let entry = entries.get(route_index).map_err(|error| {
                RouteLoadError::Abi(format!("route entry #{route_index}: {error}"))
            })?;
            let path = entry
                .get_field::<Str>("path")
                .map_err(|error| route_abi_error(route_index, "path", error))?
                .as_str()
                .map_err(|error| route_abi_error(route_index, "path", error))?
                .to_owned();
            let method_names = entry
                .get_field::<List<Str>>("method_names")
                .map_err(|error| route_abi_error(route_index, "method_names", error))?;
            let pipeline = entry
                .get_field::<Nominal<DispatchPipelineType>>("pipeline")
                .map_err(|error| route_abi_error(route_index, "pipeline", error))?;
            let controller = pipeline
                .get_callback_field::<Controller>("_dispatch")
                .map_err(|error| {
                    RouteLoadError::Abi(format!(
                        "route entry #{route_index} at {path} field pipeline._dispatch: {error}"
                    ))
                })?;
            let endpoint = Arc::new(Endpoint {
                controller,
                pipeline,
            });
            let methods = grouped.entry(path.clone()).or_default();
            let method_count = method_names
                .len()
                .map_err(|error| route_abi_error(route_index, "method_names", error))?;

            for method_index in 0..method_count {
                let name = method_names
                    .get(method_index)
                    .map_err(|error| route_abi_error(route_index, "method_names", error))?
                    .as_str()
                    .map_err(|error| route_abi_error(route_index, "method_names", error))?
                    .to_owned();
                let method = request::method_from_name(&name).map_err(|()| {
                    RouteLoadError::InvalidMethod {
                        path: path.clone(),
                        method: name.clone(),
                    }
                })?;
                if methods.contains_key(&method) {
                    return Err(RouteLoadError::DuplicateMethod {
                        path: path.clone(),
                        method: name,
                    });
                }
                methods.insert(method, Arc::clone(&endpoint));
            }
        }

        let mut matcher = PathMatcher::new();
        for (path, methods) in grouped {
            let pattern = request::matchit_pattern(&path);
            matcher
                .insert(pattern, Arc::new(PathRoute { methods }))
                .map_err(|error| RouteLoadError::ConflictingPattern(error.to_string()))?;
        }
        Ok(Self { matcher })
    }
}

fn route_abi_error(
    route_index: usize,
    field: &str,
    error: impl std::fmt::Display,
) -> RouteLoadError {
    RouteLoadError::Abi(format!("route entry #{route_index} field {field}: {error}"))
}

pub(crate) struct VutHandler {
    runtime: Runtime,
    routes: Arc<RouteTable>,
    static_files: Arc<crate::static_files::StaticFiles>,
    verbose_requests: bool,
}

impl VutHandler {
    pub(crate) fn new(
        runtime: Runtime,
        routes: RouteTable,
        static_files: crate::static_files::StaticFiles,
    ) -> Self {
        Self {
            runtime,
            routes: Arc::new(routes),
            static_files: Arc::new(static_files),
            verbose_requests: terminal::dev_verbose_requests(),
        }
    }
}

struct RequestFailure {
    stage: &'static str,
    message: String,
}

struct RequestOutcome {
    response: ServerResponse,
    failure: Option<RequestFailure>,
}

impl RequestOutcome {
    fn response(response: ServerResponse) -> Self {
        Self {
            response,
            failure: None,
        }
    }

    fn internal(stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            response: ServerResponse::text(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
            ),
            failure: Some(RequestFailure {
                stage,
                message: message.into(),
            }),
        }
    }
}

type RequestOutcomeFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = RequestOutcome> + Send>>;
type NativeOutcomeFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = (NativeResponse, Option<RequestFailure>)> + Send>,
>;

impl VutHandler {
    fn handle_detailed(&self, request: ServerRequest) -> RequestOutcomeFuture {
        let path = request::normalized_request_path(request.path());
        let Ok(matched) = self.routes.matcher.at(&path) else {
            return Box::pin(async {
                RequestOutcome::response(ServerResponse::text(StatusCode::NOT_FOUND, "Not Found"))
            });
        };
        let Some(endpoint) = matched.value.methods.get(request.method()).cloned() else {
            let mut response =
                ServerResponse::text(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed");
            let allowed = matched
                .value
                .methods
                .keys()
                .map(Method::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            if let Ok(value) = HeaderValue::from_str(&allowed) {
                response = response.with_header(ALLOW, value);
            }
            return Box::pin(async move { RequestOutcome::response(response) });
        };
        let Ok(parts) = request::build_parts(&request, &matched) else {
            return Box::pin(async {
                RequestOutcome::response(ServerResponse::text(
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                ))
            });
        };
        let runtime = match self.runtime.try_clone() {
            Ok(runtime) => runtime,
            Err(error) => {
                return Box::pin(
                    async move { RequestOutcome::internal("Runtime", error.to_string()) },
                );
            }
        };
        let peer = request.peer();

        Box::pin(async move {
            let form = crate::multipart::parse(&request).await;
            let context =
                match context::build(&runtime, &request, &parts, peer, &endpoint.pipeline, &form) {
                    Ok(context) => context,
                    Err(ContextBuildError::BadRequest) => {
                        return RequestOutcome::response(ServerResponse::text(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                        ));
                    }
                    Err(ContextBuildError::Internal) => {
                        return RequestOutcome::internal(
                            "Context construction",
                            "request ID is outside the native ABI range",
                        );
                    }
                    Err(ContextBuildError::Abi(error)) => {
                        return RequestOutcome::internal("Context construction", error.to_string());
                    }
                };
            // Context owns ABI copies; do not retain the raw/parsed Rust bodies
            // while a request waits for serialized Vut callback execution.
            drop(form);
            drop(request);
            drop(parts);

            match endpoint.controller.submit((context,)) {
                Err(error) => RequestOutcome::internal(
                    "Controller callback",
                    format!("callback submission failed: {error}"),
                ),
                Ok(operation) => match operation.await {
                    Err(error) => RequestOutcome::internal(
                        "Controller callback",
                        format!("callback execution failed: {error}"),
                    ),
                    Ok(CallbackResult::Void) => RequestOutcome::internal(
                        "Controller callback",
                        "expected a Response value but the callback returned void",
                    ),
                    Ok(CallbackResult::Value(value)) => match response::from_vut(&value) {
                        Err(error) => RequestOutcome::internal("Response conversion", error),
                        Ok(mapped) => RequestOutcome {
                            response: mapped.response,
                            failure: mapped.diagnostic.map(|message| RequestFailure {
                                stage: "View rendering",
                                message,
                            }),
                        },
                    },
                },
            }
        })
    }
}

impl Handler for VutHandler {
    fn handle_response(&self, request: ServerRequest) -> NativeResponseFuture {
        let method = request.method().clone();
        let request_path = request.path().to_owned();
        let started = Instant::now();
        // Router path matches take priority, including their 405 responses.
        // Static prefix specificity is independent of registration order.
        let path = request::normalized_request_path(request.path());
        // Classify from the handler actually selected, never from the URL suffix.
        let static_response = if self.routes.matcher.at(&path).is_err() {
            self.static_files.response(request.method(), request.path())
        } else {
            None
        };
        let (response, source): (NativeOutcomeFuture, terminal::RequestSource) =
            if let Some(response) = static_response {
                (
                    Box::pin(async move {
                        let response = response.await;
                        let failure = response.status().is_server_error().then(|| RequestFailure {
                            stage: "Static file response",
                            message: "the static file handler returned an internal error"
                                .to_owned(),
                        });
                        (response, failure)
                    }),
                    terminal::RequestSource::StaticFiles,
                )
            } else {
                let outcome = self.handle_detailed(request);
                (
                    Box::pin(async move {
                        let outcome = outcome.await;
                        (NativeResponse::Buffered(outcome.response), outcome.failure)
                    }),
                    terminal::RequestSource::Application,
                )
            };
        let verbose_requests = self.verbose_requests;
        Box::pin(async move {
            let (response, failure) = response.await;
            if let Some(failure) = failure {
                terminal::request_error(
                    &method,
                    &request_path,
                    response.status(),
                    started.elapsed(),
                    failure.stage,
                    failure.message,
                );
            } else {
                terminal::request(
                    &method,
                    &request_path,
                    response.status(),
                    started.elapsed(),
                    source,
                    verbose_requests,
                );
            }
            response
        })
    }

    fn handle(&self, request: ServerRequest) -> HandlerFuture {
        let outcome = self.handle_detailed(request);
        Box::pin(async move { outcome.await.response })
    }
}

#[derive(Debug)]
pub(crate) enum RouteLoadError {
    InvalidMethod { path: String, method: String },
    DuplicateMethod { path: String, method: String },
    ConflictingPattern(String),
    Abi(String),
}

impl std::fmt::Display for RouteLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMethod { path, method } => {
                write!(formatter, "route {path} has invalid HTTP method {method}")
            }
            Self::DuplicateMethod { path, method } => {
                write!(formatter, "duplicate route for {method} {path}")
            }
            Self::ConflictingPattern(message) => {
                write!(formatter, "conflicting route pattern: {message}")
            }
            Self::Abi(message) => write!(formatter, "could not read route table: {message}"),
        }
    }
}

impl From<matchit::InsertError> for RouteLoadError {
    fn from(error: matchit::InsertError) -> Self {
        Self::ConflictingPattern(error.to_string())
    }
}
