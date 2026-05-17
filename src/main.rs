use std::{
    env,
    io::{self, IsTerminal, Read},
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
};

const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const MARKDOWN_SYSTEM_PROMPT: &str = "\
You are a concise assistant. Answer in Markdown. Do not wrap the entire answer \
in a code block unless the user explicitly asks for raw code.";

#[derive(Debug, Parser)]
#[command(
    name = "justq",
    version,
    about = "Ask a local Ollama model one question and print the Markdown answer.",
    after_help = "Examples:
  justq \"correct English errors: bla bla bla\"
  echo \"explain this error\" | justq
  justq --model llama3:latest \"summarize Rust ownership\""
)]
struct Cli {
    #[arg(
        short,
        long,
        env = "OLLAMA_MODEL",
        default_value = "qwen2.5-coder:14b",
        help = "Ollama model name"
    )]
    model: String,

    #[arg(
        long,
        env = "OLLAMA_URL",
        value_name = "URL",
        help = "Base URL for the Ollama server; also falls back to OLLAMA_HOST"
    )]
    ollama_url: Option<String>,

    #[arg(
        value_name = "QUESTION",
        allow_hyphen_values = true,
        trailing_var_arg = true
    )]
    question: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let question = read_question(&cli.question)?;
    let ollama_url = configured_ollama_url(cli.ollama_url.as_deref());

    let ollama = Ollama::try_new(ollama_url.as_str())
        .with_context(|| format!("invalid Ollama URL: {ollama_url}"))?;
    let request = ChatMessageRequest::new(
        cli.model.clone(),
        vec![
            ChatMessage::system(MARKDOWN_SYSTEM_PROMPT.to_string()),
            ChatMessage::user(question),
        ],
    );

    let response = ollama
        .send_chat_messages(request)
        .await
        .with_context(|| format!("failed to query Ollama model {}", cli.model))?;

    println!("{}", response.message.content.trim());
    Ok(())
}

fn read_question(args: &[String]) -> Result<String> {
    let question = if args.is_empty() {
        let mut stdin = io::stdin();
        if stdin.is_terminal() {
            bail!("provide a question as arguments or pipe one on stdin");
        }

        let mut input = String::new();
        stdin
            .read_to_string(&mut input)
            .context("failed to read question from stdin")?;
        input
    } else {
        args.join(" ")
    };

    let question = question.trim().to_string();
    if question.is_empty() {
        bail!("question cannot be empty");
    }

    Ok(question)
}

fn configured_ollama_url(arg_value: Option<&str>) -> String {
    let value = arg_value
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| env::var("OLLAMA_HOST").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());

    normalize_ollama_url(&value)
}

fn normalize_ollama_url(value: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    if value.contains("://") {
        value.to_string()
    } else {
        format!("http://{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_ollama_url;

    #[test]
    fn preserves_full_urls() {
        assert_eq!(
            normalize_ollama_url("http://127.0.0.1:11434/"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn adds_http_scheme_when_missing() {
        assert_eq!(
            normalize_ollama_url("localhost:11434"),
            "http://localhost:11434"
        );
    }
}
