//! Private CLI projection of the already-flattened public route list.
//! This is not an ABI entrypoint and never inspects Vut runtime layouts.

use std::io::{self, Write as _};

use serde::Serialize;
use vut_native_sdk::{List, Nominal, Str, Value};

use crate::identity::RouteEntryType;

#[derive(Serialize)]
struct RouteDump<'a> {
    routes: &'a [RouteDescription],
}

#[derive(Serialize)]
struct RouteDescription {
    path: String,
    methods: Vec<String>,
}

pub(crate) fn write(entries: &Value<List<Nominal<RouteEntryType>>>) -> Result<(), String> {
    let count = entries
        .len()
        .map_err(|error| format!("route list length: {error}"))?;
    let mut routes = Vec::with_capacity(count);
    for index in 0..count {
        let entry = entries
            .get(index)
            .map_err(|error| format!("route entry #{index}: {error}"))?;
        let path = entry
            .get_field::<Str>("path")
            .map_err(|error| format!("route entry #{index} field path: {error}"))?
            .as_str()
            .map_err(|error| format!("route entry #{index} field path: {error}"))?
            .to_owned();
        let method_names = entry
            .get_field::<List<Str>>("method_names")
            .map_err(|error| format!("route entry #{index} field method_names: {error}"))?;
        let method_count = method_names
            .len()
            .map_err(|error| format!("route entry #{index} method count: {error}"))?;
        let mut methods = Vec::with_capacity(method_count);
        for method_index in 0..method_count {
            methods.push(
                method_names
                    .get(method_index)
                    .map_err(|error| {
                        format!("route entry #{index} method #{method_index}: {error}")
                    })?
                    .as_str()
                    .map_err(|error| {
                        format!("route entry #{index} method #{method_index}: {error}")
                    })?
                    .to_owned(),
            );
        }
        routes.push(RouteDescription { path, methods });
    }
    write_json(&routes).map_err(|error| format!("stdout: {error}"))
}

fn write_json(routes: &[RouteDescription]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, &RouteDump { routes })?;
    output.write_all(b"\n")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_dump_json_is_plain_and_keeps_method_lists() {
        let routes = vec![RouteDescription {
            path: "/admin/users/:id".to_owned(),
            methods: vec!["GET".to_owned(), "PATCH".to_owned()],
        }];
        let value = serde_json::to_value(RouteDump { routes: &routes }).unwrap();
        assert_eq!(value["routes"][0]["path"], "/admin/users/:id");
        assert_eq!(value["routes"][0]["methods"][1], "PATCH");
    }
}
