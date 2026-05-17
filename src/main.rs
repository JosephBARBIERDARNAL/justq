use std::io::{self, IsTerminal, Read};

use anyhow::{Context, Result, bail};
use clap::Parser;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
};

const MARKDOWN_SYSTEM_PROMPT: &str = "\
You are a concise assistant. Answer in Markdown. Do not wrap the entire answer \
in a code block unless the user explicitly asks for raw code.";

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Ask a local Ollama model one question and print the Markdown answer."
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
        default_value = "http://127.0.0.1:11434",
        help = "Base URL for the Ollama server"
    )]
    ollama_url: String,

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

    let ollama = Ollama::try_new(cli.ollama_url.as_str())
        .with_context(|| format!("invalid Ollama URL: {}", cli.ollama_url))?;
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
