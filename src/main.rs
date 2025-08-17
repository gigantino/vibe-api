#![deny(unused_must_use)]
mod posthog_script;

const SYSTEM_PROMPT: &str = r#"
You are a fake API response generator. Given an HTTP request, generate a realistic JSON response. The JSON response should just contain a body that makes sense for the specific response, and things like "status", "timestamp", "request_id" or other useless fields not directly related to the request shouldn't be returned.Do NOT include any markdown formatting or code blocks. Respond with raw JSON only. Strive not to return 'example.com' or something similar, you should strive for the links you are sending to be working.
    "#;

pub struct EnvVars {
    gemini_api_key: String,
    authorization_key: Option<String>,
    port: u16,
    rate_limit_max: usize,
    rate_limit_duration: u64,
    posthog_project_api_key: Option<String>,
    posthog_api_host: Option<String>,
}

pub static ENV_VARS: LazyLock<EnvVars> = LazyLock::new(|| EnvVars {
    gemini_api_key: dotenv::var("GEMINI_API_KEY").expect("Please provide a key for GEMINI_API_KEY"),
    authorization_key: dotenv::var("AUTHORIZATION_KEY").ok(),
    rate_limit_max: dotenv::var("RATE_LIMIT_MAX")
        .expect("Please provide a key for RATE_LIMIT_MAX")
        .parse()
        .expect("Please provide a rate limit that your OS supports"),
    rate_limit_duration: dotenv::var("RATE_LIMIT_DURATION")
        .expect("Please provide a key for RATE_LIMIT_DURATION")
        .parse()
        .expect("Please provide a rate limit duration of type u64"),
    posthog_project_api_key: dotenv::var("POSTHOG_PROJECT_API_KEY").ok(),
    posthog_api_host: dotenv::var("POSTHOG_API_HOST").ok(),
    port: dotenv::var("PORT")
        .unwrap_or("4069".to_string())
        .parse::<u16>()
        .expect("Please provide a VALID port"),
});

use openai_api_rs::v1::{
    api::OpenAIClient,
    chat_completion::{self, ChatCompletionMessage, ChatCompletionRequest, MessageRole},
};
use posthog_script::POSTHOG_SCRIPT;
use std::{
    fs::read_to_string,
    sync::{Arc, LazyLock},
};

use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    error::{ErrorBadRequest, ErrorInternalServerError},
    get,
    http::header::{ContentType, HeaderMap},
    middleware::{Logger, Next, from_fn},
    web::{self, Data, Html},
};
use actix_web_ratelimit::{RateLimit, config::RateLimitConfig, store::MemoryStore};
use anyhow::{Result, anyhow};
use regex::Regex;
use rusqlite::Connection;
use tokio::sync::Mutex;

pub struct State {
    db: Arc<Mutex<Connection>>,
    client: Arc<Mutex<OpenAIClient>>,
}

async fn auth_handle(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    if let Some(auth_key) = &ENV_VARS.authorization_key {
        let header_key = req
            .headers()
            .get("X-VibeApi-Authorization")
            .and_then(|key| key.to_str().ok())
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if header_key != auth_key.trim().to_lowercase() {
            return Err(ErrorBadRequest(
                "Unauthorised Request, make sure to set X-VibeApi-Authorization",
            ));
        }
    }
    next.call(req).await
}

#[actix_web::main]
async fn main() -> Result<()> {
    // database shenanigans
    let db = Arc::new(Mutex::new(Connection::open("history.db")?));
    db.lock()
        .await
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS endpoint_schemas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                endpoint_pattern TEXT UNIQUE,
                method TEXT,
                response_schema TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        ",
        )
        .expect("Error creating DB table");

    // openai style api stuff
    let api_key = &ENV_VARS.gemini_api_key;
    let client = OpenAIClient::builder()
        .with_endpoint("https://generativelanguage.googleapis.com/v1beta/openai")
        .with_api_key(api_key)
        .build()
        .expect("Failed to build the OpenAI client, did you put in the right credentials");
    let client = Arc::new(Mutex::new(client));

    let state = State { db, client };
    let state = Arc::new(state);

    let config = RateLimitConfig::default()
        .max_requests(ENV_VARS.rate_limit_max)
        .window_secs(ENV_VARS.rate_limit_duration);
    let store = Arc::new(MemoryStore::new());
    println!("Listening on port {}...", ENV_VARS.port);
    HttpServer::new(move || {
        App::new()
            .wrap(RateLimit::new(config.clone(), store.clone()))
            .wrap(Logger::default())
            .wrap(from_fn(auth_handle))
            .app_data(Data::new(state.clone()))
            .service(index)
            .default_service(web::route().to(all))
    })
    .bind(("127.0.0.1", ENV_VARS.port))?
    .run()
    .await
    .map_err(|_| anyhow!("Error Starting Server"))
}

pub static REMOVE_MARKDOWN_REG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"```(?:\w+\n)?([\s\S]*?)```"#).expect("Error compiling Regex"));
pub fn remove_markdown(input: &str) -> &str {
    let regex = &REMOVE_MARKDOWN_REG;
    let Some(caps) = regex.captures(input) else {
        return input;
    };
    return caps
        .get(1)
        .map(|m| m.as_str().trim())
        .unwrap_or_else(|| input);
}

#[get("/")]
async fn index() -> impl actix_web::Responder {
    let mut html_content = read_to_string("./public/index.html")
        .unwrap_or(r#"<h1 color="red">Failed to get HTML</h1>"#.to_string());

    if let (Some(posthog_key), Some(posthog_host)) = (
        &ENV_VARS.posthog_project_api_key,
        &ENV_VARS.posthog_api_host,
    ) {
        html_content = html_content
            .replace("<!-- POSTHOG-PLACEHOLDER -->", POSTHOG_SCRIPT)
            .replace(r#"${Bun.env.POSTHOG_PROJECT_API_KEY}"#, &posthog_key)
            .replace(r#"${Bun.env.POSTHOG_API_HOST}"#, &posthog_host);
    }
    Html::new(html_content)
}

pub async fn all(
    req: HttpRequest,
    body: web::Bytes,
    state: Data<Arc<State>>,
) -> Result<HttpResponse, actix_web::Error> {
    let db = state.db.clone();
    let headers = req.headers();
    let headers: HeaderMap = headers
        .into_iter()
        .filter_map(|(headername, value)| {
            let lower_key = headername.as_str().to_lowercase();
            if lower_key == "x-vibeapi-authorization" || lower_key == "x-vibeapi-refresh" {
                return None;
            }
            Some((headername.to_owned(), value.to_owned()))
        })
        .collect();
    let url = req.full_url();
    let endpoint_pattern = url.path();
    let is_authorised = req
        .headers()
        .get("X-VibeApi-Refresh")
        .and_then(|value| {
            Some(
                value.to_str().unwrap_or_default()
                    == &ENV_VARS.authorization_key.clone().unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    let refresh_header = req
        .headers()
        .get("X-VibeApi-Refresh")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let force_regen_structure = is_authorised && refresh_header == "true";
    let method = req.method();

    let mut example_prompt = "".to_string();

    let existing_schema = if !force_regen_structure {
        (db.lock().await.query_one(
            "
                SELECT response_schema
                FROM endpoint_schemas
                WHERE endpoint_pattern = (?1)
                AND method = (?2)
            ",
            [endpoint_pattern, method.as_str()],
            |row| row.get::<_, String>(0),
        ))
        .ok()
    } else {
        None
    };

    if let Some(existing_schema) = &existing_schema {
        example_prompt = format!(r#"This endpoint has a specific schema that MUST be followed: {existing_schema} You MUST use exactly this schema structure, changing only the values to be appropriate for the current request parameters. The field names and nested structure must remain identical."#)
        .to_string();
    }

    let body = String::from_utf8(body.to_vec()).unwrap_or_default();
    let user_prompt = format!(
        r#"
Generate a fake API response based on the following request:

- Method: {}
- Path: {}
- Headers: {}
- Body: {}

{}

Reply ONLY with raw JSON. No explanation. No markdown."#,
        req.method(),
        endpoint_pattern,
        headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k.as_str(), v.to_str().unwrap_or_default()))
            .collect::<Vec<String>>()
            .join("\n"),
        body,
        example_prompt
    );

    let client = state.client.clone();
    let messages = vec![
        (MessageRole::system, SYSTEM_PROMPT),
        (MessageRole::user, &user_prompt),
    ]
    .into_iter()
    .map(|(role, content)| ChatCompletionMessage {
        role,
        content: chat_completion::Content::Text(content.to_string()),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    })
    .collect();
    let openai_req = ChatCompletionRequest::new("gemini-2.0-flash".to_string(), messages);
    let mut client = client.lock().await;
    let completion = client
        .chat_completion(openai_req)
        .await
        .map_err(|_| ErrorInternalServerError("Failed to get completion"))?;
    let response_text = &completion.choices[0].message.content.clone();
    let Some(response_text) = response_text else {
        return Ok(HttpResponse::NotFound().body("No response text found"));
    };

    let vibe_response = remove_markdown(&response_text);

    if force_regen_structure {
        db.lock()
            .await
            .execute(
                "
             INSERT OR REPLACE INTO endpoint_schemas (endpoint_pattern, method, response_schema)
             VALUES (?1, ?2, ?3)
            ",
                [endpoint_pattern, method.as_str(), vibe_response],
            )
            .map_err(|_| ErrorInternalServerError("Failed to insert schema into DB"))?;
    }

    if existing_schema.is_none() {
        db.lock()
            .await
            .execute(
                "
             INSERT OR IGNORE INTO endpoint_schemas (endpoint_pattern, method, response_schema)
             VALUES (?1, ?2, ?3)
            ",
                [endpoint_pattern, method.as_str(), vibe_response],
            )
            .map_err(|_| ErrorInternalServerError("Failed to insert schema into DB"))?;
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(vibe_response.to_string()))
}
