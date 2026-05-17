use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use arboard::Clipboard;
use clap::{Args, Parser, Subcommand, ValueEnum};
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
};

const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const MARKDOWN_SYSTEM_PROMPT: &str = "\
You are a precise bilingual writing assistant for French and English. \
Return only the requested text, with no introduction, \
explanation, quotation marks, or surrounding code block.";
const SPINNER_FRAMES: [&str; 4] = ["-", "\\", "|", "/"];
const SPINNER_MESSAGES: [&str; 8] = [
    "Polishing the sentence",
    "Looking for the mot juste",
    "Negotiating with accents",
    "Untangling grammar",
    "Sharpening the wording",
    "Keeping the tone intact",
    "Checking both languages",
    "Warming up the local model",
];

#[derive(Debug, Parser)]
#[command(
    name = "justq",
    version,
    about = "Translate or correct French/English text with a local Ollama model.",
    after_help = "Examples:
  justq correct \"i has a apple\"
  justq translate \"bonjour tout le monde\"
  justq translate --to french \"hello world\"
  justq --no-copy correct \"je suis aller au bureau\""
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
        long,
        global = true,
        help = "Do not copy the raw model output to the system clipboard"
    )]
    no_copy: bool,

    #[arg(
        long,
        global = true,
        help = "Print only the raw Markdown output, without terminal formatting"
    )]
    raw: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(
        alias = "t",
        about = "Translate between French and English",
        after_help = "Examples:
  justq translate \"bonjour tout le monde\"
  justq translate --to english \"bonjour tout le monde\"
  echo \"hello world\" | justq translate --to french"
    )]
    Translate(TranslateCommand),

    #[command(
        alias = "fix",
        about = "Correct French or English text",
        after_help = "Examples:
  justq correct \"i has a apple\"
  justq correct --language french \"je suis aller au bureau\"
  echo \"i has a apple\" | justq correct --no-copy"
    )]
    Correct(CorrectCommand),
}

#[derive(Debug, Args)]
struct TranslateCommand {
    #[arg(
        short,
        long,
        value_enum,
        help = "Target language; if omitted, justq translates to the other language"
    )]
    to: Option<Language>,

    #[arg(value_name = "TEXT")]
    text: Vec<String>,
}

#[derive(Debug, Args)]
struct CorrectCommand {
    #[arg(
        short,
        long,
        value_enum,
        help = "Text language; if omitted, the model detects French or English"
    )]
    language: Option<Language>,

    #[arg(value_name = "TEXT")]
    text: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Language {
    #[value(alias = "en")]
    English,
    #[value(alias = "fr")]
    French,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let input = read_text(cli.command.text_args())?;
    let user_prompt = cli.command.prompt(&input);
    let ollama_url = configured_ollama_url(cli.ollama_url.as_deref());

    let ollama = Ollama::try_new(ollama_url.as_str())
        .with_context(|| format!("invalid Ollama URL: {ollama_url}"))?;
    let request = ChatMessageRequest::new(
        cli.model.clone(),
        vec![
            ChatMessage::system(MARKDOWN_SYSTEM_PROMPT.to_string()),
            ChatMessage::user(user_prompt),
        ],
    );

    let spinner = Spinner::start();
    let response = ollama.send_chat_messages(request).await;
    drop(spinner);

    let response =
        response.with_context(|| format!("failed to query Ollama model {}", cli.model))?;
    let output = response.message.content.trim();

    print_output(output, cli.raw);

    if !cli.no_copy {
        match copy_to_clipboard(output) {
            Ok(()) => print_status("Copied to clipboard."),
            Err(error) => print_status(&format!("Could not copy to clipboard: {error:#}")),
        }
    }

    Ok(())
}

impl Command {
    fn text_args(&self) -> &[String] {
        match self {
            Self::Translate(command) => &command.text,
            Self::Correct(command) => &command.text,
        }
    }

    fn prompt(&self, input: &str) -> String {
        match self {
            Self::Translate(command) => translate_prompt(command.to, input),
            Self::Correct(command) => correct_prompt(command.language, input),
        }
    }
}

impl Language {
    fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::French => "French",
        }
    }
}

fn translate_prompt(target_language: Option<Language>, input: &str) -> String {
    let target_instruction = match target_language {
        Some(language) => format!("Translate the text into {}.", language.label()),
        None => {
            "Detect whether the text is French or English, then translate it into the other language."
                .to_string()
        }
    };

    format!(
        "{target_instruction} Preserve meaning, tone, line breaks, and Markdown formatting. \
Return only the translated text.\n\nText:\n{input}"
    )
}

fn correct_prompt(language: Option<Language>, input: &str) -> String {
    let language_instruction = match language {
        Some(language) => format!("The text is in {}.", language.label()),
        None => "Detect whether the text is French or English.".to_string(),
    };

    format!(
        "{language_instruction} Correct grammar, spelling, punctuation, and wording errors. \
Preserve the original language, meaning, tone, line breaks, and Markdown formatting. \
Return only the corrected text.\n\nText:\n{input}"
    )
}

fn read_text(args: &[String]) -> Result<String> {
    let text = if args.is_empty() {
        let mut stdin = io::stdin();
        if stdin.is_terminal() {
            bail!("provide text as arguments or pipe it on stdin");
        }

        let mut input = String::new();
        stdin
            .read_to_string(&mut input)
            .context("failed to read text from stdin")?;
        input
    } else {
        args.join(" ")
    };

    let text = text.trim().to_string();
    if text.is_empty() {
        bail!("text cannot be empty");
    }

    Ok(text)
}

fn copy_to_clipboard(output: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("failed to access the system clipboard")?;
    clipboard
        .set_text(output.to_string())
        .context("failed to copy output to the system clipboard")
}

struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    fn start() -> Self {
        if !io::stderr().is_terminal() {
            return Self {
                running: Arc::new(AtomicBool::new(false)),
                handle: None,
            };
        }

        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let message = random_spinner_message();
        let handle = thread::spawn(move || {
            let mut stderr = io::stderr();
            let mut frame_index = 0;

            while thread_running.load(Ordering::Relaxed) {
                let frame = SPINNER_FRAMES[frame_index % SPINNER_FRAMES.len()];
                let _ = write!(stderr, "\r\x1b[2m{frame} {message}...\x1b[0m");
                let _ = stderr.flush();
                frame_index += 1;
                thread::sleep(Duration::from_millis(120));
            }

            let _ = write!(stderr, "\r\x1b[2K");
            let _ = stderr.flush();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn random_spinner_message() -> &'static str {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let index = seed as usize % SPINNER_MESSAGES.len();

    SPINNER_MESSAGES[index]
}

fn print_output(output: &str, raw: bool) {
    if raw || !io::stdout().is_terminal() {
        println!("{output}");
        return;
    }

    println!("{output}");
}

fn print_status(message: &str) {
    if io::stderr().is_terminal() {
        eprintln!("\x1b[2m{message}\x1b[0m");
    } else {
        eprintln!("{message}");
    }
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
    use super::{Language, correct_prompt, normalize_ollama_url, translate_prompt};

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

    #[test]
    fn translate_prompt_can_force_target_language() {
        let prompt = translate_prompt(Some(Language::French), "hello");

        assert!(prompt.contains("Translate the text into French."));
        assert!(prompt.contains("Text:\nhello"));
    }

    #[test]
    fn correct_prompt_can_force_language() {
        let prompt = correct_prompt(Some(Language::English), "i has a apple");

        assert!(prompt.contains("The text is in English."));
        assert!(prompt.contains("Return only the corrected text."));
    }
}
