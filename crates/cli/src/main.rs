use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Json,
    Human,
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Http {
        step: &'static str,
        status_code: Option<u16>,
        message: String,
    },
    MissingField {
        step: &'static str,
        field: &'static str,
    },
}

#[derive(Serialize)]
struct StepResult {
    action: &'static str,
    status_code: u16,
    response: Value,
}

#[derive(Serialize)]
struct ErrorOutput {
    code: &'static str,
    step: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_code: Option<u16>,
    message: String,
}

#[derive(Serialize)]
struct SuccessOutput {
    ok: bool,
    flow: &'static str,
    steps: Vec<StepResult>,
}

#[derive(Serialize)]
struct FailureOutput {
    ok: bool,
    flow: &'static str,
    error: ErrorOutput,
}

fn main() {
    let _ = domain::domain_ready();

    let (mode, base_url) = match parse_args(std::env::args().skip(1).collect()) {
        Ok(v) => v,
        Err(err) => {
            render_error(OutputMode::Json, err);
            std::process::exit(2);
        }
    };

    match run_scripted_core_flow(&base_url) {
        Ok(steps) => render_success(mode, steps),
        Err(err) => {
            render_error(mode, err);
            std::process::exit(1);
        }
    }
}

fn parse_args(args: Vec<String>) -> Result<(OutputMode, String), CliError> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(CliError::Usage(
            "usage: cli [--output json|human] [--base-url URL] flow scripted-core".to_string(),
        ));
    }

    let mut mode = OutputMode::Json;
    let mut base_url = String::from("http://127.0.0.1:3000");

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--output" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::Usage("missing value for --output".to_string()))?;
                mode = match value.as_str() {
                    "json" => OutputMode::Json,
                    "human" => OutputMode::Human,
                    _ => {
                        return Err(CliError::Usage(
                            "invalid --output, expected json|human".to_string(),
                        ))
                    }
                };
                i += 2;
            }
            "--base-url" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::Usage("missing value for --base-url".to_string()))?;
                base_url = value.clone();
                i += 2;
            }
            "--db-path" => {
                i += 2;
            }
            "flow" => {
                let flow = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::Usage("missing flow name".to_string()))?;
                if flow != "scripted-core" {
                    return Err(CliError::Usage(
                        "only `flow scripted-core` is supported".to_string(),
                    ));
                }
                return Ok((mode, base_url));
            }
            other => {
                return Err(CliError::Usage(format!("unknown argument `{other}`")));
            }
        }
    }

    Err(CliError::Usage(
        "expected: [--output json|human] [--base-url URL] flow scripted-core".to_string(),
    ))
}

fn run_scripted_core_flow(base_url: &str) -> Result<Vec<StepResult>, CliError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let mut steps = Vec::new();

    steps.push(post(
        &agent,
        base_url,
        "create_category",
        "/categories",
        json!({"slug":"network","name":"Network"}),
    )?);

    steps.push(post(
        &agent,
        base_url,
        "create_tag_definition_text",
        "/tag-definitions",
        json!({"tag_key":"owner","display_name":"Owner","value_type":"text"}),
    )?);
    let owner_text_tag_definition_id = required_i64(&steps[1], "create_tag_definition_text", "id")?;

    let enum_td = post(
        &agent,
        base_url,
        "create_tag_definition_enum",
        "/tag-definitions",
        json!({"tag_key":"status","display_name":"Status","value_type":"enum"}),
    )?;
    let enum_tag_definition_id = required_i64(&enum_td, "create_tag_definition_enum", "id")?;
    steps.push(enum_td);

    steps.push(post(
        &agent,
        base_url,
        "create_enum_option",
        "/tag-enum-options",
        json!({"tag_definition_id":enum_tag_definition_id,"option_key":"active","display_name":"Active","sort_order":0}),
    )?);

    let ext_type = post(
        &agent,
        base_url,
        "create_external_entity_type",
        "/external-entity-types",
        json!({"type_key":"vendor","display_name":"Vendor"}),
    )?;
    let external_entity_type_id = required_i64(&ext_type, "create_external_entity_type", "id")?;
    steps.push(ext_type);

    steps.push(post(
        &agent,
        base_url,
        "create_external_entity",
        "/external-entities",
        json!({"external_entity_type_id":external_entity_type_id,"external_key":"v-1","display_name":"Acme"}),
    )?);

    let external_td = post(
        &agent,
        base_url,
        "create_tag_definition_external_entity",
        "/tag-definitions",
        json!({
            "tag_key":"vendor_ref",
            "display_name":"Vendor",
            "value_type":"external_entity",
            "external_entity_type_id":external_entity_type_id
        }),
    )?;
    steps.push(external_td);

    let category = &steps[0];
    let category_id = required_i64(category, "create_category", "id")?;

    steps.push(post(
        &agent,
        base_url,
        "create_event_type",
        "/event-types",
        json!({
            "event_type_id":"asset.set-owner",
            "display_name":"Set Owner",
            "mutations":[{"operation":"set","tag_definition_id":owner_text_tag_definition_id,"input_key":"owner"}]
        }),
    )?);

    let asset = post(
        &agent,
        base_url,
        "create_asset",
        "/assets",
        json!({"category_id":category_id,"asset_tag":"AST-FLOW-001"}),
    )?;
    let asset_tag = asset
        .response
        .get("asset_tag")
        .and_then(Value::as_str)
        .unwrap_or("AST-FLOW-001")
        .to_string();
    steps.push(asset);

    steps.push(post_with_headers(
        &agent,
        base_url,
        "apply_event",
        &format!("/assets/{asset_tag}/events"),
        json!({"event_type_id":"asset.set-owner","payload":{"owner":"team-a"}}),
        &[("Idempotency-Key", "ham-flow-001")],
    )?);

    steps.push(get(
        &agent,
        base_url,
        "fetch_timeline",
        &format!("/assets/{asset_tag}/events?limit=10"),
    )?);

    steps.push(post(
        &agent,
        base_url,
        "run_search",
        "/assets/search",
        json!({"filters":[{"field":"asset_tag","op":"eq","value":asset_tag}]}),
    )?);

    Ok(steps)
}

fn post(
    agent: &ureq::Agent,
    base_url: &str,
    action: &'static str,
    path: &str,
    body: Value,
) -> Result<StepResult, CliError> {
    post_with_headers(agent, base_url, action, path, body, &[])
}

fn post_with_headers(
    agent: &ureq::Agent,
    base_url: &str,
    action: &'static str,
    path: &str,
    body: Value,
    headers: &[(&str, &str)],
) -> Result<StepResult, CliError> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);
    let mut req = agent.post(&url).set("content-type", "application/json");
    for (name, value) in headers {
        req = req.set(name, value);
    }

    match req.send_string(&body.to_string()) {
        Ok(resp) => Ok(StepResult {
            action,
            status_code: resp.status(),
            response: parse_response_body(resp),
        }),
        Err(ureq::Error::Status(status, resp)) => Err(CliError::Http {
            step: action,
            status_code: Some(status),
            message: parse_response_body(resp).to_string(),
        }),
        Err(err) => Err(CliError::Http {
            step: action,
            status_code: None,
            message: err.to_string(),
        }),
    }
}

fn get(
    agent: &ureq::Agent,
    base_url: &str,
    action: &'static str,
    path: &str,
) -> Result<StepResult, CliError> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);
    match agent.get(&url).call() {
        Ok(resp) => Ok(StepResult {
            action,
            status_code: resp.status(),
            response: parse_response_body(resp),
        }),
        Err(ureq::Error::Status(status, resp)) => Err(CliError::Http {
            step: action,
            status_code: Some(status),
            message: parse_response_body(resp).to_string(),
        }),
        Err(err) => Err(CliError::Http {
            step: action,
            status_code: None,
            message: err.to_string(),
        }),
    }
}

fn parse_response_body(resp: ureq::Response) -> Value {
    use std::io::Read;
    let mut s = String::new();
    if resp.into_reader().read_to_string(&mut s).is_ok() {
        if s.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s))
        }
    } else {
        Value::Null
    }
}

fn render_success(mode: OutputMode, steps: Vec<StepResult>) {
    match mode {
        OutputMode::Json => println!(
            "{}",
            serde_json::to_string_pretty(&SuccessOutput {
                ok: true,
                flow: "scripted-core",
                steps,
            })
            .unwrap()
        ),
        OutputMode::Human => {
            for (idx, step) in steps.iter().enumerate() {
                println!(
                    "{:02} {} status={} keys={}",
                    idx + 1,
                    step.action,
                    step.status_code,
                    top_level_keys(&step.response)
                );
            }
            println!("DONE flow=scripted-core");
        }
    }
}

fn render_error(mode: OutputMode, err: CliError) {
    let body = match err {
        CliError::Usage(message) => FailureOutput {
            ok: false,
            flow: "scripted-core",
            error: ErrorOutput {
                code: "USAGE",
                step: "parse_args",
                status_code: None,
                message,
            },
        },
        CliError::Http {
            step,
            status_code,
            message,
        } => FailureOutput {
            ok: false,
            flow: "scripted-core",
            error: ErrorOutput {
                code: "HTTP_ERROR",
                step,
                status_code,
                message,
            },
        },
        CliError::MissingField { step, field } => FailureOutput {
            ok: false,
            flow: "scripted-core",
            error: ErrorOutput {
                code: "INVALID_RESPONSE",
                step,
                status_code: None,
                message: format!("missing required field `{field}`"),
            },
        },
    };

    match mode {
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(&body).unwrap()),
        OutputMode::Human => eprintln!(
            "ERROR code={} step={} status={:?} message={}",
            body.error.code, body.error.step, body.error.status_code, body.error.message
        ),
    }
}

fn required_i64(
    step: &StepResult,
    step_name: &'static str,
    field: &'static str,
) -> Result<i64, CliError> {
    step.response
        .get(field)
        .and_then(Value::as_i64)
        .ok_or(CliError::MissingField {
            step: step_name,
            field,
        })
}

fn top_level_keys(value: &Value) -> String {
    value
        .as_object()
        .map(|m| {
            let mut k: Vec<&str> = m.keys().map(String::as_str).collect();
            k.sort_unstable();
            k.join(",")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{parse_args, CliError, OutputMode};

    #[test]
    fn parse_args_accepts_help_flag() {
        let parsed = parse_args(vec!["--help".to_string()]);
        assert!(matches!(
            parsed,
            Err(CliError::Usage(message)) if message.contains("usage:")
        ));
    }

    #[test]
    fn parse_args_accepts_short_help_flag() {
        let parsed = parse_args(vec!["-h".to_string()]);
        assert!(matches!(
            parsed,
            Err(CliError::Usage(message)) if message.contains("usage:")
        ));
    }

    #[test]
    fn parse_args_still_parses_flow() {
        let parsed = parse_args(vec![
            "--output".to_string(),
            "human".to_string(),
            "flow".to_string(),
            "scripted-core".to_string(),
        ])
        .unwrap();

        assert!(matches!(parsed.0, OutputMode::Human));
        assert_eq!(parsed.1, "http://127.0.0.1:3000");
    }
}
