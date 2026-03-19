use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutputMode {
    Json,
    Human,
}

#[derive(Debug, Parser)]
#[command(name = "cli", about = "HAM scripted CLI client")]
struct CliArgs {
    #[arg(long, value_enum, default_value_t = OutputMode::Json)]
    output: OutputMode,

    #[arg(long, default_value = "http://127.0.0.1:3000")]
    base_url: String,

    #[arg(long, hide = true)]
    db_path: Option<String>,

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    Flow {
        #[command(subcommand)]
        flow: FlowCommand,
    },
    Category {
        #[command(subcommand)]
        category: CategoryCommand,
    },
    Asset {
        #[command(subcommand)]
        asset: AssetCommand,
    },
}

#[derive(Debug, Subcommand)]
enum FlowCommand {
    #[command(name = "scripted-core")]
    ScriptedCore,
}

#[derive(Debug, Subcommand)]
enum CategoryCommand {
    Create(CategoryCreateArgs),
    List,
    Delete(CategoryDeleteArgs),
}

#[derive(Debug, Args)]
struct CategoryCreateArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    parent_id: Option<i64>,
}

#[derive(Debug, Args)]
struct CategoryDeleteArgs {
    #[arg(long)]
    id: i64,
}

#[derive(Debug, Subcommand)]
enum AssetCommand {
    Create(AssetCreateArgs),
    Get(AssetGetArgs),
    List(AssetListArgs),
    Update(AssetUpdateArgs),
    Delete(AssetDeleteArgs),
}

#[derive(Debug, Args)]
struct AssetCreateArgs {
    #[arg(long)]
    category_id: i64,
    #[arg(long)]
    asset_tag: Option<String>,
}

#[derive(Debug, Args)]
struct AssetGetArgs {
    #[arg(long)]
    id: i64,
    #[arg(long)]
    include_deleted: bool,
}

#[derive(Debug, Args)]
struct AssetListArgs {
    #[arg(long)]
    include_deleted: bool,
}

#[derive(Debug, Args)]
struct AssetUpdateArgs {
    #[arg(long)]
    id: i64,
    #[arg(long)]
    display_name: Option<String>,
    #[arg(long)]
    clear_display_name: bool,
}

#[derive(Debug, Args)]
struct AssetDeleteArgs {
    #[arg(long)]
    id: i64,
}

#[derive(Debug)]
enum CliError {
    Http {
        step: &'static str,
        status_code: Option<u16>,
        message: String,
    },
    Validation {
        step: &'static str,
        message: String,
    },
    MissingField {
        step: &'static str,
        field: &'static str,
    },
    UnsupportedCommand {
        command: &'static str,
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

#[derive(Serialize)]
struct CommandSuccessOutput {
    ok: bool,
    command: &'static str,
    result: Value,
}

#[derive(Serialize)]
struct CommandFailureOutput {
    ok: bool,
    command: &'static str,
    error: ErrorOutput,
}

#[derive(Serialize)]
struct UnsupportedCommandOutput {
    ok: bool,
    error: UnsupportedCommandErrorOutput,
}

#[derive(Serialize)]
struct UnsupportedCommandErrorOutput {
    code: &'static str,
    command: &'static str,
    message: &'static str,
}

fn main() {
    let _ = domain::domain_ready();

    let cli = CliArgs::parse();
    let _ = &cli.db_path;
    let mode = cli.output;
    let base_url = cli.base_url;

    let exit_code = run_command(mode, &base_url, cli.command);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run_command(mode: OutputMode, base_url: &str, command: CliCommand) -> i32 {
    match command {
        CliCommand::Flow {
            flow: FlowCommand::ScriptedCore,
        } => match run_scripted_core_flow(&base_url) {
            Ok(steps) => {
                render_success(mode, steps);
                0
            }
            Err(err) => {
                render_error(mode, err);
                1
            }
        },
        CliCommand::Category { category } => match category {
            CategoryCommand::Create(args) => match run_category_create(base_url, args) {
                Ok(result) => {
                    render_command_success(mode, "category create", result);
                    0
                }
                Err(err) => {
                    render_command_error(mode, "category create", err);
                    1
                }
            },
            CategoryCommand::List => match run_category_list(base_url) {
                Ok(result) => {
                    render_command_success(mode, "category list", result);
                    0
                }
                Err(err) => {
                    render_command_error(mode, "category list", err);
                    1
                }
            },
            CategoryCommand::Delete(args) => match run_category_delete(base_url, args) {
                Ok(result) => {
                    render_command_success(mode, "category delete", result);
                    0
                }
                Err(err) => {
                    render_command_error(mode, "category delete", err);
                    1
                }
            },
        },
        CliCommand::Asset { asset } => {
            match asset {
                AssetCommand::Create(args) => match run_asset_create(base_url, args) {
                    Ok(result) => {
                        render_command_success(mode, "asset create", result);
                        0
                    }
                    Err(err) => {
                        render_command_error(mode, "asset create", err);
                        1
                    }
                },
                AssetCommand::Get(args) => match run_asset_get(base_url, args) {
                    Ok(result) => {
                        render_command_success(mode, "asset get", result);
                        0
                    }
                    Err(err) => {
                        render_command_error(mode, "asset get", err);
                        1
                    }
                },
                AssetCommand::List(args) => match run_asset_list(base_url, args) {
                    Ok(result) => {
                        render_command_success(mode, "asset list", result);
                        0
                    }
                    Err(err) => {
                        render_command_error(mode, "asset list", err);
                        1
                    }
                },
                AssetCommand::Update(args) => match run_asset_update(base_url, args) {
                    Ok(result) => {
                        render_command_success(mode, "asset update", result);
                        0
                    }
                    Err(err) => {
                        render_command_error(mode, "asset update", err);
                        1
                    }
                },
                AssetCommand::Delete(args) => match run_asset_delete(base_url, args) {
                    Ok(result) => {
                        render_command_success(mode, "asset delete", result);
                        0
                    }
                    Err(err) => {
                        render_command_error(mode, "asset delete", err);
                        1
                    }
                },
            }
        }
    }
}

fn run_asset_create(base_url: &str, args: AssetCreateArgs) -> Result<Value, CliError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let body = match args.asset_tag {
        Some(asset_tag) => json!({"category_id": args.category_id, "asset_tag": asset_tag}),
        None => json!({"category_id": args.category_id}),
    };

    let result = post(&agent, base_url, "asset_create", "/assets", body)?;
    Ok(result.response)
}

fn run_asset_get(base_url: &str, args: AssetGetArgs) -> Result<Value, CliError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let mut path = format!("/assets/{}", args.id);
    if args.include_deleted {
        path.push_str("?include_deleted=true");
    }

    let result = get(&agent, base_url, "asset_get", &path)?;
    Ok(result.response)
}

fn run_asset_list(base_url: &str, args: AssetListArgs) -> Result<Value, CliError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let mut path = String::from("/assets");
    if args.include_deleted {
        path.push_str("?include_deleted=true");
    }

    let result = get(&agent, base_url, "asset_list", &path)?;
    Ok(result.response)
}

fn run_asset_update(base_url: &str, args: AssetUpdateArgs) -> Result<Value, CliError> {
    if args.display_name.is_some() && args.clear_display_name {
        return Err(CliError::Validation {
            step: "asset_update",
            message: "--display-name and --clear-display-name cannot be used together"
                .to_string(),
        });
    }

    let body = if let Some(display_name) = args.display_name {
        json!({"display_name": display_name})
    } else if args.clear_display_name {
        json!({"clear_display_name": true})
    } else {
        return Err(CliError::Validation {
            step: "asset_update",
            message: "at least one update field is required (--display-name or --clear-display-name)"
                .to_string(),
        });
    };

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();
    let path = format!("/assets/{}", args.id);
    let result = patch(&agent, base_url, "asset_update", &path, body)?;
    Ok(result.response)
}

fn run_asset_delete(base_url: &str, args: AssetDeleteArgs) -> Result<Value, CliError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let result = delete_by_id(&agent, base_url, "asset_delete", "/assets", args.id)?;
    Ok(result.response)
}

fn run_category_create(base_url: &str, args: CategoryCreateArgs) -> Result<Value, CliError> {
    let name = args.name.trim();
    if name.is_empty() {
        return Err(CliError::Validation {
            step: "category_create",
            message: "name must not be blank".to_string(),
        });
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let body = match args.parent_id {
        Some(parent_category_id) => {
            json!({"name": name, "parent_category_id": parent_category_id})
        }
        None => json!({"name": name}),
    };

    let result = post(&agent, base_url, "category_create", "/categories", body)?;

    Ok(result.response)
}

fn run_category_list(base_url: &str) -> Result<Value, CliError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let result = get(&agent, base_url, "category_list", "/categories")?;
    Ok(result.response)
}

fn run_category_delete(base_url: &str, args: CategoryDeleteArgs) -> Result<Value, CliError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let result = delete_by_id(&agent, base_url, "category_delete", "/categories", args.id)?;
    Ok(result.response)
}

fn asset_command_label(command: &AssetCommand) -> &'static str {
    match command {
        AssetCommand::Create(_) => "asset.create",
        AssetCommand::Get(_) => "asset.get",
        AssetCommand::List(_) => "asset.list",
        AssetCommand::Update(_) => "asset.update",
        AssetCommand::Delete(_) => "asset.delete",
    }
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
        json!({"name":"Network"}),
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

fn patch(
    agent: &ureq::Agent,
    base_url: &str,
    action: &'static str,
    path: &str,
    body: Value,
) -> Result<StepResult, CliError> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);
    match agent
        .request("PATCH", &url)
        .set("content-type", "application/json")
        .send_string(&body.to_string())
    {
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

fn delete_by_id(
    agent: &ureq::Agent,
    base_url: &str,
    action: &'static str,
    collection_path: &str,
    id: i64,
) -> Result<StepResult, CliError> {
    let path = format!("{}/{}", collection_path.trim_end_matches('/'), id);
    delete(agent, base_url, action, &path)
}

fn delete(
    agent: &ureq::Agent,
    base_url: &str,
    action: &'static str,
    path: &str,
) -> Result<StepResult, CliError> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);
    match agent.delete(&url).call() {
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
        CliError::Validation { step, message } => FailureOutput {
            ok: false,
            flow: "scripted-core",
            error: ErrorOutput {
                code: "VALIDATION_ERROR",
                step,
                status_code: None,
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
        CliError::UnsupportedCommand { command } => {
            match mode {
                OutputMode::Json => {
                    println!("{}", format_unsupported_command_json(command));
                }
                OutputMode::Human => {
                    eprintln!("{}", format_unsupported_command_human(command));
                }
            }
            return;
        }
    };

    match mode {
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(&body).unwrap()),
        OutputMode::Human => eprintln!(
            "ERROR code={} step={} status={:?} message={}",
            body.error.code, body.error.step, body.error.status_code, body.error.message
        ),
    }
}

fn render_command_success(mode: OutputMode, command: &'static str, result: Value) {
    match mode {
        OutputMode::Json => println!(
            "{}",
            serde_json::to_string_pretty(&CommandSuccessOutput {
                ok: true,
                command,
                result,
            })
            .unwrap()
        ),
        OutputMode::Human => println!("DONE command={} keys={}", command, top_level_keys(&result)),
    }
}

fn render_command_error(mode: OutputMode, command: &'static str, err: CliError) {
    let error = match err {
        CliError::Http {
            step,
            status_code,
            message,
        } => ErrorOutput {
            code: "HTTP_ERROR",
            step,
            status_code,
            message,
        },
        CliError::Validation { step, message } => ErrorOutput {
            code: "VALIDATION_ERROR",
            step,
            status_code: None,
            message,
        },
        CliError::MissingField { step, field } => ErrorOutput {
            code: "INVALID_RESPONSE",
            step,
            status_code: None,
            message: format!("missing required field `{field}`"),
        },
        CliError::UnsupportedCommand { command } => {
            match mode {
                OutputMode::Json => {
                    println!("{}", format_unsupported_command_json(command));
                }
                OutputMode::Human => {
                    eprintln!("{}", format_unsupported_command_human(command));
                }
            }
            return;
        }
    };

    let body = CommandFailureOutput {
        ok: false,
        command,
        error,
    };

    match mode {
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(&body).unwrap()),
        OutputMode::Human => eprintln!(
            "ERROR command={} code={} step={} status={:?} message={}",
            body.command,
            body.error.code,
            body.error.step,
            body.error.status_code,
            body.error.message
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

fn unsupported_command_output(command: &'static str) -> UnsupportedCommandOutput {
    UnsupportedCommandOutput {
        ok: false,
        error: UnsupportedCommandErrorOutput {
            code: "NOT_IMPLEMENTED",
            command,
            message: "command is parsed but not implemented yet; use `flow scripted-core`",
        },
    }
}

fn format_unsupported_command_json(command: &'static str) -> String {
    serde_json::to_string_pretty(&unsupported_command_output(command)).unwrap()
}

fn format_unsupported_command_human(command: &'static str) -> String {
    let payload = unsupported_command_output(command);
    format!(
        "ERROR code={} command={} message={}",
        payload.error.code, payload.error.command, payload.error.message
    )
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
    use super::{
        AssetCommand, AssetCreateArgs, AssetDeleteArgs, AssetGetArgs, AssetListArgs,
        AssetUpdateArgs, CategoryCommand, CategoryCreateArgs, CategoryDeleteArgs, CliArgs,
        CliCommand, FlowCommand, OutputMode,
    };
    use axum::{extract::Path, routing::{delete, get}, Json, Router};
    use clap::{error::ErrorKind, Parser};
    use serde_json::json;
    use tokio::{net::TcpListener, time::Duration};

    #[test]
    fn parse_supports_help_flag() {
        let err = CliArgs::try_parse_from(["cli", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn parse_rejects_missing_subcommand() {
        let err = CliArgs::try_parse_from(["cli", "--output", "human"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingSubcommand);
    }

    #[test]
    fn parse_scripted_core_flow_and_options() {
        let parsed = CliArgs::try_parse_from([
            "cli",
            "--output",
            "human",
            "--base-url",
            "http://example.test:8080",
            "flow",
            "scripted-core",
        ])
        .unwrap();

        assert!(matches!(parsed.output, OutputMode::Human));
        assert_eq!(parsed.base_url, "http://example.test:8080");
        assert!(matches!(
            parsed.command,
            CliCommand::Flow {
                flow: FlowCommand::ScriptedCore
            }
        ));
    }

    #[test]
    fn parse_category_create_with_parent_id() {
        let parsed = CliArgs::try_parse_from([
            "cli",
            "category",
            "create",
            "--name",
            "Servers",
            "--parent-id",
            "10",
        ])
        .unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Category {
                category: CategoryCommand::Create(CategoryCreateArgs {
                    name,
                    parent_id: Some(10)
                })
            } if name == "Servers"
        ));
    }

    #[test]
    fn parse_category_create_rejects_non_numeric_parent_id() {
        let err = CliArgs::try_parse_from([
            "cli",
            "category",
            "create",
            "--name",
            "Servers",
            "--parent-id",
            "not-a-number",
        ])
        .unwrap_err();

        let rendered = err.to_string();
        assert!(rendered.contains("--parent-id"));
        assert!(rendered.contains("not-a-number"));
    }

    #[test]
    fn parse_category_list() {
        let parsed = CliArgs::try_parse_from(["cli", "category", "list"]).unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Category {
                category: CategoryCommand::List
            }
        ));
    }

    #[test]
    fn parse_category_delete() {
        let parsed = CliArgs::try_parse_from(["cli", "category", "delete", "--id", "42"]).unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Category {
                category: CategoryCommand::Delete(CategoryDeleteArgs { id: 42 })
            }
        ));
    }

    #[test]
    fn parse_category_delete_rejects_positional_id() {
        let err = CliArgs::try_parse_from(["cli", "category", "delete", "42"]).unwrap_err();

        let rendered = err.to_string();
        assert!(rendered.contains("42"));
        assert!(rendered.contains("--id"));
    }

    #[test]
    fn parse_asset_create() {
        let parsed = CliArgs::try_parse_from([
            "cli",
            "asset",
            "create",
            "--category-id",
            "3",
            "--asset-tag",
            "AST-100",
        ])
        .unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Asset {
                asset: AssetCommand::Create(AssetCreateArgs {
                    category_id: 3,
                    asset_tag: Some(asset_tag)
                })
            } if asset_tag == "AST-100"
        ));
    }

    #[test]
    fn parse_asset_create_without_optional_asset_tag() {
        let parsed =
            CliArgs::try_parse_from(["cli", "asset", "create", "--category-id", "3"]).unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Asset {
                asset: AssetCommand::Create(AssetCreateArgs {
                    category_id: 3,
                    asset_tag: None
                })
            }
        ));
    }

    #[test]
    fn parse_asset_create_rejects_display_name() {
        let err = CliArgs::try_parse_from([
            "cli",
            "asset",
            "create",
            "--category-id",
            "3",
            "--display-name",
            "Core Router",
        ])
        .unwrap_err();

        let rendered = err.to_string();
        assert!(rendered.contains("--display-name"));
    }

    #[test]
    fn parse_asset_get_with_include_deleted() {
        let parsed =
            CliArgs::try_parse_from(["cli", "asset", "get", "--id", "9", "--include-deleted"])
                .unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Asset {
                asset: AssetCommand::Get(AssetGetArgs {
                    id: 9,
                    include_deleted: true
                })
            }
        ));
    }

    #[test]
    fn parse_asset_get_rejects_positional_id() {
        let err = CliArgs::try_parse_from(["cli", "asset", "get", "9"]).unwrap_err();

        let rendered = err.to_string();
        assert!(rendered.contains("9"));
        assert!(rendered.contains("--id"));
    }

    #[test]
    fn parse_asset_list_with_include_deleted() {
        let parsed =
            CliArgs::try_parse_from(["cli", "asset", "list", "--include-deleted"]).unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Asset {
                asset: AssetCommand::List(AssetListArgs {
                    include_deleted: true
                })
            }
        ));
    }

    #[test]
    fn parse_asset_update_display_name() {
        let parsed = CliArgs::try_parse_from([
            "cli",
            "asset",
            "update",
            "--id",
            "9",
            "--display-name",
            "Core Router",
        ])
        .unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Asset {
                asset: AssetCommand::Update(AssetUpdateArgs {
                    id: 9,
                    display_name: Some(name),
                    clear_display_name: false
                })
            } if name == "Core Router"
        ));
    }

    #[test]
    fn parse_asset_update_clear_display_name() {
        let parsed = CliArgs::try_parse_from([
            "cli",
            "asset",
            "update",
            "--id",
            "9",
            "--clear-display-name",
        ])
        .unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Asset {
                asset: AssetCommand::Update(AssetUpdateArgs {
                    id: 9,
                    display_name: None,
                    clear_display_name: true
                })
            }
        ));
    }

    #[test]
    fn parse_asset_update_rejects_positional_id() {
        let err = CliArgs::try_parse_from([
            "cli",
            "asset",
            "update",
            "9",
            "--display-name",
            "Core Router",
        ])
        .unwrap_err();

        let rendered = err.to_string();
        assert!(rendered.contains("9"));
        assert!(rendered.contains("--id"));
    }

    #[test]
    fn parse_asset_delete() {
        let parsed = CliArgs::try_parse_from(["cli", "asset", "delete", "--id", "9"]).unwrap();

        assert!(matches!(
            parsed.command,
            CliCommand::Asset {
                asset: AssetCommand::Delete(AssetDeleteArgs { id: 9 })
            }
        ));
    }

    #[test]
    fn parse_asset_delete_rejects_positional_id() {
        let err = CliArgs::try_parse_from(["cli", "asset", "delete", "9"]).unwrap_err();

        let rendered = err.to_string();
        assert!(rendered.contains("9"));
        assert!(rendered.contains("--id"));
    }

    #[test]
    fn run_command_unsupported_returns_controlled_error_exit_code() {
        let exit_code = super::run_command(
            OutputMode::Json,
            "http://example.test",
            CliCommand::Asset {
                asset: AssetCommand::List(AssetListArgs {
                    include_deleted: false,
                }),
            },
        );

        assert_eq!(exit_code, 2);
    }

    #[tokio::test]
    async fn category_list_gets_categories() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        let app = Router::new().route(
            "/categories",
            get(|| async {
                Json(json!({
                    "items": [
                        {"id": 1, "name": "Network", "parent_category_id": null}
                    ]
                }))
            }),
        );

        let _join = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        wait_for_server(&base_url).await;

        let exit_code = tokio::task::spawn_blocking(move || {
            super::run_command(
                OutputMode::Json,
                &base_url,
                CliCommand::Category {
                    category: CategoryCommand::List,
                },
            )
        })
        .await
        .unwrap();

        assert_eq!(exit_code, 0);
    }

    #[tokio::test]
    async fn category_delete_calls_delete_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        let app = Router::new().route(
            "/categories/:id",
            delete(|Path(id): Path<i64>| async move {
                assert_eq!(id, 42);
                Json(json!({"ok": true}))
            }),
        );

        let _join = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        wait_for_server(&base_url).await;

        let exit_code = tokio::task::spawn_blocking(move || {
            super::run_command(
                OutputMode::Json,
                &base_url,
                CliCommand::Category {
                    category: CategoryCommand::Delete(CategoryDeleteArgs { id: 42 }),
                },
            )
        })
        .await
        .unwrap();

        assert_eq!(exit_code, 0);
    }

    async fn wait_for_server(base_url: &str) {
        let host = base_url.trim_start_matches("http://");
        for _ in 0..50 {
            if std::net::TcpStream::connect(host).is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("server at {base_url} did not become ready in time");
    }

    #[test]
    fn render_error_json_for_non_flow_is_stable() {
        let rendered = super::format_unsupported_command_json("category.list");
        let parsed = serde_json::from_str::<serde_json::Value>(&rendered).unwrap();

        assert_eq!(
            parsed,
            json!({
                "ok": false,
                "error": {
                    "code": "NOT_IMPLEMENTED",
                    "command": "category.list",
                    "message": "command is parsed but not implemented yet; use `flow scripted-core`"
                }
            })
        );
    }

    #[test]
    fn render_error_human_for_non_flow_is_stable() {
        let rendered = super::format_unsupported_command_human("asset.delete");
        assert_eq!(
            rendered,
            "ERROR code=NOT_IMPLEMENTED command=asset.delete message=command is parsed but not implemented yet; use `flow scripted-core`"
        );
    }
}
