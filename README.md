# justa-question

A tiny Rust CLI that asks a local Ollama model one question and prints the answer
as Markdown.

The installed command is `justq`.

## Requirements

- Rust 1.85 or newer
- Ollama running locally
- A local model, for example:

```bash
ollama pull qwen2.5-coder:14b
```

## Install

```bash
cargo install --git https://github.com/y-sunflower/justa-question
```

## Usage

Ask a question:

```bash
justq "correct English errors: bla bla bla"
```

Pipe a question from stdin:

```bash
echo "correct English errors: i has a apple" | justq
```

Use another model:

```bash
justq --model llama3:latest "explain Rust ownership in 5 bullets"
```

Use another Ollama server:

```bash
justq --ollama-url http://localhost:11434 "summarize this"
```

Configuration can also come from environment variables:

```bash
OLLAMA_MODEL=llama3:latest justq "write a git commit message"
OLLAMA_HOST=http://localhost:11434 justq "what is a borrow checker?"
OLLAMA_URL=http://localhost:11434 justq "what is a borrow checker?"
```

## Short Alias

The project does not install a binary named `jq` by default because that would
conflict with the popular JSON processor. If you do not use that command, add a
local shell alias:

```bash
alias jq='justq'
```

Then you can run:

```bash
jq "correct English errors: bla bla bla"
```

## Build From Source

```bash
git clone https://github.com/y-sunflower/justa-question.git
cd justa-question
cargo build --release
```
