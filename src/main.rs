use std::{
    env,
    io::{self, IsTerminal, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use arboard::Clipboard;
use clap::{Args, Parser, Subcommand, ValueEnum};
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, ChatMessageResponseStream, request::ChatMessageRequest},
};
use termimad::MadSkin;
use tokio_stream::StreamExt;

const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_MODEL: &str = "qwen2.5-coder:14b";
const WRITING_SYSTEM_PROMPT: &str = "\
You are a precise bilingual writing assistant for French and English. \
Return only the requested text, with no introduction, \
explanation, quotation marks, or surrounding code block.";
const ASK_SYSTEM_PROMPT: &str = "\
You are a senior software development assistant. \
Answer the user's question directly with practical software engineering guidance. \
Use concise Markdown with short sections, bullets when useful, and fenced code blocks for code. \
Do not translate or correct the question unless explicitly asked.\
The question might not be about software development, in this case adapt to it.";
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPINNER_MESSAGES: [&str; 8] = [
    "Asking the model",
    "Reading the prompt",
    "Thinking through it",
    "Drafting the answer",
    "Checking the details",
    "Keeping it concise",
    "Formatting Markdown",
    "Waiting for Ollama",
];
const MARKDOWN_RENDER_INTERVAL: Duration = Duration::from_millis(60);

#[derive(Debug, Parser)]
#[command(
    name = "justq",
    version,
    about = "Ask software development questions, translate, or correct French/English text with a local Ollama model.",
    after_help = "Examples:
  justq correct \"i has a apple\"
  justq translate \"bonjour tout le monde\"
  justq ask \"how do I handle errors in Rust?\"
  justq translate --to french \"hello world\"
  justq --no-copy correct \"je suis aller au bureau\""
)]
struct Cli {
    #[arg(
        short,
        long,
        value_name = "MODEL",
        help = "Ollama model name; defaults to JUSTQ_MODEL, then OLLAMA_MODEL, then qwen2.5-coder:14b"
    )]
    model: Option<String>,

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
        help = "Do not copy translation or correction output to the system clipboard"
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

    #[command(
        alias = "a",
        about = "Ask a concise software development question",
        after_help = "Examples:
  justq ask \"how do I handle errors in Rust?\"
  justq ask \"explain when to use Arc<Mutex<T>>\"
  echo \"why is this borrow checker error happening?\" | justq ask"
    )]
    Ask(AskCommand),
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

#[derive(Debug, Args)]
struct AskCommand {
    #[arg(value_name = "QUESTION")]
    question: Vec<String>,
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
    let system_prompt = cli.command.system_prompt();
    let user_prompt = cli.command.prompt(&input);
    let model = configured_model(cli.model.as_deref());
    let ollama_url = configured_ollama_url(cli.ollama_url.as_deref());

    let ollama = Ollama::try_new(ollama_url.as_str())
        .with_context(|| format!("invalid Ollama URL: {ollama_url}"))?;
    let request = ChatMessageRequest::new(
        model.clone(),
        vec![
            ChatMessage::system(system_prompt.to_string()),
            ChatMessage::user(user_prompt),
        ],
    );

    if cli.command.streams_output() {
        stream_output(&ollama, request, &model, cli.raw).await?;
        return Ok(());
    }

    let spinner = Spinner::start();
    let response = ollama.send_chat_messages(request).await;
    drop(spinner);

    let response = response.with_context(|| format!("failed to query Ollama model {model}"))?;
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
            Self::Ask(command) => &command.question,
        }
    }

    fn system_prompt(&self) -> &'static str {
        match self {
            Self::Translate(_) | Self::Correct(_) => WRITING_SYSTEM_PROMPT,
            Self::Ask(_) => ASK_SYSTEM_PROMPT,
        }
    }

    fn prompt(&self, input: &str) -> String {
        match self {
            Self::Translate(command) => translate_prompt(command.to, input),
            Self::Correct(command) => correct_prompt(command.language, input),
            Self::Ask(_) => ask_prompt(input),
        }
    }

    fn streams_output(&self) -> bool {
        matches!(self, Self::Ask(_))
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
        "{target_instruction} Preserve meaning, tone and line breaks. \
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
Preserve the original language, meaning, tone, line breaks, and keep as much as possible original style. \
Return only the corrected text.\n\nText:\n{input}"
    )
}

fn ask_prompt(input: &str) -> String {
    format!(
        "Answer this software development question. \
Be concise, practical, and write valid Markdown.\n\nQuestion:\n{input}"
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

async fn stream_output(
    ollama: &Ollama,
    request: ChatMessageRequest,
    model: &str,
    raw: bool,
) -> Result<()> {
    let spinner = Spinner::start();
    let stream = ollama.send_chat_messages_stream(request).await;
    drop(spinner);

    let mut stream = stream.with_context(|| format!("failed to query Ollama model {model}"))?;

    if raw || !io::stdout().is_terminal() {
        return stream_raw_output(&mut stream, model).await;
    }

    stream_rendered_markdown(&mut stream, model).await
}

async fn stream_raw_output(stream: &mut ChatMessageResponseStream, model: &str) -> Result<()> {
    let mut stdout = io::stdout().lock();
    let mut printed_anything = false;
    let mut ends_with_newline = false;

    while let Some((content, _done)) = next_stream_chunk(stream, model).await? {
        if content.is_empty() {
            continue;
        }

        stdout
            .write_all(content.as_bytes())
            .context("failed to write streamed output")?;
        stdout.flush().context("failed to flush streamed output")?;

        printed_anything = true;
        ends_with_newline = content.ends_with('\n');
    }

    if printed_anything && !ends_with_newline {
        stdout
            .write_all(b"\n")
            .context("failed to write streamed output terminator")?;
    }

    Ok(())
}

async fn stream_rendered_markdown(
    stream: &mut ChatMessageResponseStream,
    model: &str,
) -> Result<()> {
    let skin = MadSkin::default();
    let mut stdout = io::stdout().lock();
    let mut markdown = String::new();
    let mut rendered_lines = 0;
    let mut needs_render = false;
    let mut last_rendered_at: Option<Instant> = None;

    while let Some((content, done)) = next_stream_chunk(stream, model).await? {
        if !content.is_empty() {
            markdown.push_str(&content);
            needs_render = true;
        }

        let should_render = last_rendered_at
            .map(|instant| instant.elapsed() >= MARKDOWN_RENDER_INTERVAL)
            .unwrap_or(true);
        if needs_render && (done || should_render) {
            rendered_lines =
                render_markdown_preview(&mut stdout, &skin, markdown.trim_end(), rendered_lines)?;
            needs_render = false;
            last_rendered_at = Some(Instant::now());
        }
    }

    if needs_render {
        render_markdown_preview(&mut stdout, &skin, markdown.trim_end(), rendered_lines)?;
    }

    Ok(())
}

async fn next_stream_chunk(
    stream: &mut ChatMessageResponseStream,
    model: &str,
) -> Result<Option<(String, bool)>> {
    match stream.next().await {
        Some(Ok(response)) => Ok(Some((response.message.content, response.done))),
        Some(Err(())) => bail!("failed to stream response from Ollama model {model}"),
        None => Ok(None),
    }
}

fn render_markdown_preview<W: Write>(
    writer: &mut W,
    skin: &MadSkin,
    markdown: &str,
    previous_lines: usize,
) -> Result<usize> {
    if previous_lines > 0 {
        write!(writer, "\x1b[{previous_lines}A\x1b[J")
            .context("failed to clear previous Markdown preview")?;
    }

    if markdown.is_empty() {
        writer.flush().context("failed to flush Markdown preview")?;
        return Ok(0);
    }

    let text = skin.term_text(markdown);
    let rendered_lines = text.lines.len();
    write!(writer, "{text}").context("failed to write Markdown preview")?;
    writer.flush().context("failed to flush Markdown preview")?;

    Ok(rendered_lines)
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
    let index = fastrand::usize(..SPINNER_MESSAGES.len());

    SPINNER_MESSAGES[index]
}

fn print_output(output: &str, raw: bool) {
    if raw || !io::stdout().is_terminal() {
        println!("{output}");
        return;
    }

    let skin = MadSkin::default();
    skin.print_text(output);
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

fn configured_model(arg_value: Option<&str>) -> String {
    let justq_model = env::var("JUSTQ_MODEL").ok();
    let ollama_model = env::var("OLLAMA_MODEL").ok();

    select_model(arg_value, justq_model.as_deref(), ollama_model.as_deref())
}

fn select_model(
    arg_value: Option<&str>,
    justq_model: Option<&str>,
    ollama_model: Option<&str>,
) -> String {
    [arg_value, justq_model, ollama_model]
        .into_iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
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
    use super::{
        ASK_SYSTEM_PROMPT, Language, ask_prompt, correct_prompt, normalize_ollama_url,
        select_model, translate_prompt,
    };

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

    #[test]
    fn ask_prompt_targets_software_development_markdown() {
        let prompt = ask_prompt("when should I use Result?");

        assert!(prompt.contains("software development question"));
        assert!(prompt.contains("write valid Markdown"));
        assert!(prompt.contains("Question:\nwhen should I use Result?"));
        assert!(ASK_SYSTEM_PROMPT.contains("software development assistant"));
    }

    #[test]
    fn model_argument_wins_over_env_values() {
        assert_eq!(
            select_model(
                Some("cli-model"),
                Some("justq-env-model"),
                Some("ollama-env-model")
            ),
            "cli-model"
        );
    }

    #[test]
    fn justq_model_wins_over_ollama_model() {
        assert_eq!(
            select_model(None, Some("justq-env-model"), Some("ollama-env-model")),
            "justq-env-model"
        );
    }

    #[test]
    fn ollama_model_remains_supported() {
        assert_eq!(
            select_model(None, None, Some("ollama-env-model")),
            "ollama-env-model"
        );
    }
}
